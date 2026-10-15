//! Small helpers over `sq_core::ir::Expr`: conjunct splitting, slot / EXISTS collection and
//! classification of the filter shapes the rewrites care about.
use sq_core::batch::{Slot, SlotSet};
use sq_core::id::{Tag, TermId};
use sq_core::ir::{ExistsId, Expr};

/// Calls `f` on `e` and every sub-expression (pre-order). Does not enter EXISTS patterns.
pub(crate) fn visit(e: &Expr, f: &mut impl FnMut(&Expr)) {
    f(e);
    match e {
        Expr::Const(_) | Expr::Slot(_) | Expr::Bound(_) | Expr::Exists(_) => {}
        Expr::Not(a) | Expr::UnaryPlus(a) | Expr::UnaryMinus(a) => visit(a, f),
        Expr::Or(a, b)
        | Expr::And(a, b)
        | Expr::Equal(a, b)
        | Expr::SameTerm(a, b)
        | Expr::Greater(a, b)
        | Expr::GreaterOrEqual(a, b)
        | Expr::Less(a, b)
        | Expr::LessOrEqual(a, b)
        | Expr::Add(a, b)
        | Expr::Subtract(a, b)
        | Expr::Multiply(a, b)
        | Expr::Divide(a, b) => {
            visit(a, f);
            visit(b, f);
        }
        Expr::In(a, l) => {
            visit(a, f);
            l.iter().for_each(|x| visit(x, f));
        }
        Expr::If(a, b, c) => {
            visit(a, f);
            visit(b, f);
            visit(c, f);
        }
        Expr::Coalesce(l) | Expr::Call(_, l) => l.iter().for_each(|x| visit(x, f)),
    }
}

/// Slots read directly by `e` (not inside EXISTS patterns).
pub(crate) fn slots(e: &Expr) -> SlotSet {
    let mut v = Vec::new();
    e.slots(&mut v);
    SlotSet::from_slots(v)
}

/// The EXISTS patterns referenced by `e`, in pre-order.
pub(crate) fn exists_ids(e: &Expr) -> Vec<ExistsId> {
    let mut out = Vec::new();
    visit(e, &mut |x| {
        if let Expr::Exists(id) = x {
            out.push(*id);
        }
    });
    out
}

/// Split a conjunction into its top-level conjuncts (left to right).
pub(crate) fn conjuncts(e: Expr, out: &mut Vec<Expr>) {
    match e {
        Expr::And(a, b) => {
            conjuncts(*a, out);
            conjuncts(*b, out);
        }
        other => out.push(other),
    }
}

/// Rebuild a left-deep conjunction (None for an empty list).
pub(crate) fn and_all(list: Vec<Expr>) -> Option<Expr> {
    list.into_iter().reduce(|a, b| Expr::And(Box::new(a), Box::new(b)))
}

/// Is `id` a constant for which SPARQL `=` coincides with `sameTerm` inside a FILTER
/// (an IRI or a simple literal; spec §6.3 / §7.3.3)? Errors count as false in a FILTER, so
/// `?x = c` and `sameTerm(?x, c)` select exactly the same rows for such a `c`.
pub(crate) fn eq_is_same_term(id: TermId) -> bool {
    let tag = if id.is_temp() { id.temp_tag() } else { id.tag() };
    matches!(tag, Tag::Iri | Tag::Str | Tag::ShortStr)
}

/// `FILTER(sameTerm(?x, c))`, `FILTER(sameTerm(c, ?x))`, and `?x = c` / `c = ?x` for an IRI
/// or simple-literal `c`: returns `(x, c)`.
pub(crate) fn seed_constant(e: &Expr) -> Option<(Slot, TermId)> {
    let (a, b, eq) = match e {
        Expr::SameTerm(a, b) => (a, b, false),
        Expr::Equal(a, b) => (a, b, true),
        _ => return None,
    };
    let (slot, c) = match (&**a, &**b) {
        (Expr::Slot(s), Expr::Const(c)) | (Expr::Const(c), Expr::Slot(s)) => (*s, *c),
        _ => return None,
    };
    if !c.is_bound() || (eq && !eq_is_same_term(c)) {
        return None;
    }
    Some((slot, c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::codec;

    fn slot(s: Slot) -> Box<Expr> {
        Box::new(Expr::Slot(s))
    }
    fn cst(id: TermId) -> Box<Expr> {
        Box::new(Expr::Const(id))
    }

    #[test]
    fn conjunct_round_trip() {
        let e = Expr::And(
            Box::new(Expr::And(Box::new(Expr::Bound(1)), Box::new(Expr::Bound(2)))),
            Box::new(Expr::Bound(3)),
        );
        let mut out = Vec::new();
        conjuncts(e.clone(), &mut out);
        assert_eq!(out, vec![Expr::Bound(1), Expr::Bound(2), Expr::Bound(3)]);
        assert_eq!(and_all(out), Some(e));
        assert_eq!(and_all(vec![]), None);
    }

    #[test]
    fn seed_constant_shapes() {
        let iri = TermId::from_dict(2000, Tag::Iri);
        let short = codec::encode_short_str("abc").unwrap();
        let long = TermId::from_dict(2001, Tag::Str);
        let lang = TermId::from_dict(2002, Tag::Lang);
        let int = TermId::from_int(3).unwrap();
        let temp_iri = TermId::temp(0, Tag::Iri);
        assert_eq!(seed_constant(&Expr::SameTerm(slot(4), cst(int))), Some((4, int)));
        assert_eq!(seed_constant(&Expr::SameTerm(cst(lang), slot(4))), Some((4, lang)));
        assert_eq!(seed_constant(&Expr::Equal(slot(1), cst(iri))), Some((1, iri)));
        assert_eq!(seed_constant(&Expr::Equal(slot(1), cst(short))), Some((1, short)));
        assert_eq!(seed_constant(&Expr::Equal(slot(1), cst(long))), Some((1, long)));
        assert_eq!(seed_constant(&Expr::Equal(slot(1), cst(temp_iri))), Some((1, temp_iri)));
        // value equality differs from sameTerm for numbers and language-tagged literals
        assert_eq!(seed_constant(&Expr::Equal(slot(1), cst(int))), None);
        assert_eq!(seed_constant(&Expr::Equal(slot(1), cst(lang))), None);
        assert_eq!(seed_constant(&Expr::Equal(slot(1), slot(2))), None);
    }

    #[test]
    fn slots_and_exists() {
        let e = Expr::Or(
            Box::new(Expr::Exists(ExistsId(3))),
            Box::new(Expr::Not(Box::new(Expr::Exists(ExistsId(1))))),
        );
        assert_eq!(exists_ids(&e), vec![ExistsId(3), ExistsId(1)]);
        assert!(slots(&e).is_empty());
        let e = Expr::Call(spargebra::algebra::Function::Str, vec![Expr::Slot(7), Expr::Bound(9)]);
        assert_eq!(slots(&e), SlotSet::from_slots([7, 9]));
    }
}
