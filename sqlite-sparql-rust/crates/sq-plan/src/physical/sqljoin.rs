//! The SqlJoin hybrid (spec §7.5, §7.6): a left-deep chain of index-nested-loop joins over
//! plain quad patterns, rendered as ONE SQL statement that SQLite's VDBE steps.
//!
//! ```sql
//! SELECT t0.s, t1.o FROM main.rdf_quad AS t0 INDEXED BY rdf_gpos
//!   CROSS JOIN main.rdf_quad AS t1 INDEXED BY rdf_gpso
//!  WHERE t0.g=?1 AND t0.p=?2 AND t0.o=?3 AND t1.g=?4 AND t1.p=?5 AND t1.s=t0.s
//! ```
//! `CROSS JOIN` makes SQLite keep OUR join order and `INDEXED BY` our access paths: alias i
//! reads the order the planner chose for the pattern with the slots of aliases 0..i bound, so
//! every alias is a `SEARCH ... USING COVERING INDEX` on exactly that prefix (verified with
//! `EXPLAIN QUERY PLAN` on SQLite 3.45.1). Every constant (graph included) is a parameter, so
//! the text depends only on the chain's shape and is shared by the statement pool. A slot
//! becomes an output column at its first occurrence; every later occurrence (in the same alias
//! or a later one) is an equality with it.
use std::fmt::Write;

use sq_core::batch::Slot;
use sq_core::ir::{DatasetSpec, PatTerm, QuadPat};
use sq_core::plan::{JoinKind, Op, Plan};
use sq_core::store::{Col, IndexOrder};

use super::scan::fixed_graph_id;

/// A rendered SqlJoin statement: SQL text, parameter values (`?1..?n`) and the slot of each
/// result column.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rendered {
    pub sql: String,
    pub params: Vec<i64>,
    pub out: Vec<Slot>,
}

const COL_NAMES: [&str; 4] = ["g", "s", "p", "o"];

fn col_name(c: Col) -> &'static str {
    COL_NAMES[c as usize]
}

/// `main.rdf_quad AS t{i} INDEXED BY <index of order>`.
fn from_item(order: IndexOrder, alias: usize) -> String {
    let from = order.sql_from();
    match from.split_once(" INDEXED BY ") {
        Some((table, index)) => format!("{table} AS t{alias} INDEXED BY {index}"),
        None => format!("{from} AS t{alias}"),
    }
}

/// Render a chain of quad patterns, each with the index order it is read through. None when a
/// pattern does not read one fixed stored graph, or when the chain binds no slot.
pub(crate) fn render(aliases: &[(QuadPat, IndexOrder)], dataset: &DatasetSpec) -> Option<Rendered> {
    let mut select = String::new();
    let mut from = String::new();
    let mut terms: Vec<String> = Vec::new();
    let mut params: Vec<i64> = Vec::new();
    let mut out: Vec<Slot> = Vec::new();
    let mut first_seen: Vec<(Slot, usize, Col)> = Vec::new();
    for (i, (pat, order)) in aliases.iter().enumerate() {
        if i > 0 {
            from.push_str(" CROSS JOIN ");
        }
        from.push_str(&from_item(*order, i));
        for col in order.cols() {
            let term = match col {
                Col::G => PatTerm::Const(fixed_graph_id(pat, dataset)?),
                Col::S => pat.s,
                Col::P => pat.p,
                Col::O => pat.o,
            };
            let lhs = format!("t{i}.{}", col_name(col));
            match term {
                PatTerm::Const(c) => {
                    params.push(c.raw());
                    terms.push(format!("{lhs}=?{}", params.len()));
                }
                PatTerm::Slot(s) => match first_seen.iter().find(|(x, _, _)| *x == s) {
                    Some(&(_, j, c)) => terms.push(format!("{lhs}=t{j}.{}", col_name(c))),
                    None => {
                        first_seen.push((s, i, col));
                        if !out.is_empty() {
                            select.push_str(", ");
                        }
                        select.push_str(&lhs);
                        out.push(s);
                    }
                },
            }
        }
    }
    if out.is_empty() {
        return None;
    }
    let mut sql = format!("SELECT {select} FROM {from}");
    if !terms.is_empty() {
        let _ = write!(sql, " WHERE {}", terms.join(" AND "));
    }
    Some(Rendered { sql, params, out })
}

/// The scans of a left-deep chain of inner BindJoins over plain scans (what SQL-chain join
/// steps build), in join order; None for any other shape.
pub(crate) fn chain_scans(p: &Plan) -> Option<Vec<(QuadPat, IndexOrder)>> {
    let scan = |p: &Plan| match &p.op {
        Op::Scan(s) if s.range.is_none() => Some((s.pat, s.order)),
        _ => None,
    };
    match &p.op {
        Op::BindJoin { left, right, kind: JoinKind::Inner, .. } => {
            let mut v = chain_scans(left)?;
            v.push(scan(right)?);
            Some(v)
        }
        _ => Some(vec![scan(p)?]),
    }
}

/// Right-hand side of a WHERE term.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Operand {
    /// `?n` (1-based).
    Param(usize),
    /// `t{alias}.{col}`.
    Column(usize, Col),
}

/// One alias of a parsed statement: its index order and its WHERE terms, in text order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ParsedAlias {
    pub order: IndexOrder,
    pub terms: Vec<(Col, Operand)>,
}

/// A statement in the shape [`render`] produces (EXPLAIN output and `validate()` read it).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Parsed {
    pub select: Vec<(usize, Col)>,
    pub aliases: Vec<ParsedAlias>,
    /// Number of `?n` parameters (they are numbered 1..=n in text order).
    pub params: usize,
}

fn parse_col(s: &str) -> Option<Col> {
    Some(match s {
        "g" => Col::G,
        "s" => Col::S,
        "p" => Col::P,
        "o" => Col::O,
        _ => return None,
    })
}

/// `t{i}.{c}`.
fn parse_column(s: &str) -> Option<(usize, Col)> {
    let (alias, col) = s.strip_prefix('t')?.split_once('.')?;
    Some((alias.parse().ok()?, parse_col(col)?))
}

/// Parse a statement produced by [`render`]; None for any other text.
pub(crate) fn parse(sql: &str) -> Option<Parsed> {
    let rest = sql.strip_prefix("SELECT ")?;
    let (select, rest) = rest.split_once(" FROM ")?;
    let (from, where_) = match rest.split_once(" WHERE ") {
        Some((f, w)) => (f, Some(w)),
        None => (rest, None),
    };
    let select = select.split(", ").map(parse_column).collect::<Option<Vec<_>>>()?;
    let mut aliases = Vec::new();
    for (i, item) in from.split(" CROSS JOIN ").enumerate() {
        let (table, index) = item.split_once(&format!(" AS t{i} INDEXED BY "))?;
        let wanted = format!("{table} INDEXED BY {index}");
        let full_g_first = |o: &IndexOrder| !o.named_only() && !o.numeric_exceptions();
        let order = IndexOrder::ALL.into_iter().find(|o| o.sql_from() == wanted && full_g_first(o))?;
        aliases.push(ParsedAlias { order, terms: Vec::new() });
    }
    let mut params = 0;
    for term in where_.into_iter().flat_map(|w| w.split(" AND ")) {
        let (lhs, rhs) = term.split_once('=')?;
        let (alias, col) = parse_column(lhs)?;
        let operand = match rhs.strip_prefix('?') {
            Some(n) => {
                let n: usize = n.parse().ok()?;
                params += 1;
                if n != params {
                    return None;
                }
                Operand::Param(n)
            }
            None => {
                let (a, c) = parse_column(rhs)?;
                if a > alias {
                    return None;
                }
                Operand::Column(a, c)
            }
        };
        aliases.get_mut(alias)?.terms.push((col, operand));
    }
    if select.iter().any(|(a, _)| *a >= aliases.len()) {
        return None;
    }
    Some(Parsed { select, aliases, params })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::id::Tag;
    use sq_core::ir::{DefaultGraph, GraphSel, NamedGraphs};
    use sq_core::TermId;

    fn iri(n: i64) -> TermId {
        TermId::from_dict(2000 + n, Tag::Iri)
    }

    fn pat(s: PatTerm, p: PatTerm, o: PatTerm) -> QuadPat {
        QuadPat { g: GraphSel::Default, s, p, o }
    }

    fn ds() -> DatasetSpec {
        DatasetSpec { default: DefaultGraph::StoreDefault, named: NamedGraphs::All }
    }

    #[test]
    fn star_renders_in_join_order() {
        let (x, a) = (PatTerm::Slot(0), PatTerm::Slot(1));
        let chain = [
            (pat(x, PatTerm::Const(iri(1)), PatTerm::Const(iri(2))), IndexOrder::Gpos),
            (pat(x, PatTerm::Const(iri(3)), a), IndexOrder::Gpso),
            (pat(x, PatTerm::Const(iri(4)), a), IndexOrder::Gspo),
        ];
        let r = render(&chain, &ds()).unwrap();
        assert_eq!(
            r.sql,
            "SELECT t0.s, t1.o FROM main.rdf_quad AS t0 INDEXED BY rdf_gpos \
             CROSS JOIN main.rdf_quad AS t1 INDEXED BY rdf_gpso \
             CROSS JOIN main.rdf_quad AS t2 INDEXED BY sqlite_autoindex_rdf_quad_1 \
             WHERE t0.g=?1 AND t0.p=?2 AND t0.o=?3 AND t1.g=?4 AND t1.p=?5 AND t1.s=t0.s \
             AND t2.g=?6 AND t2.s=t0.s AND t2.p=?7 AND t2.o=t1.o"
        );
        assert_eq!(r.params, vec![0, iri(1).raw(), iri(2).raw(), 0, iri(3).raw(), 0, iri(4).raw()]);
        assert_eq!(r.out, vec![0, 1]);
        let parsed = parse(&r.sql).unwrap();
        assert_eq!(parsed.select, vec![(0, Col::S), (1, Col::O)]);
        assert_eq!(parsed.params, 7);
        assert_eq!(parsed.aliases.iter().map(|a| a.order).collect::<Vec<_>>(), vec![
            IndexOrder::Gpos,
            IndexOrder::Gpso,
            IndexOrder::Gspo
        ]);
        assert_eq!(parsed.aliases[2].terms[1], (Col::S, Operand::Column(0, Col::S)));
    }

    #[test]
    fn repeated_slots_and_unsupported_graphs() {
        let x = PatTerm::Slot(0);
        let r = render(&[(pat(x, PatTerm::Slot(1), x), IndexOrder::Gspo)], &ds()).unwrap();
        assert_eq!(
            r.sql,
            "SELECT t0.s, t0.p FROM main.rdf_quad AS t0 INDEXED BY sqlite_autoindex_rdf_quad_1 \
             WHERE t0.g=?1 AND t0.o=t0.s"
        );
        assert_eq!(r.out, vec![0, 1]);
        let active = QuadPat { g: GraphSel::Active, ..pat(x, PatTerm::Slot(1), PatTerm::Slot(2)) };
        assert!(render(&[(active, IndexOrder::Gspo)], &ds()).is_none());
        let ground = pat(PatTerm::Const(iri(1)), PatTerm::Const(iri(2)), PatTerm::Const(iri(3)));
        assert!(render(&[(ground, IndexOrder::Gspo)], &ds()).is_none(), "no output column");
        assert!(parse("SELECT t0.s FROM main.rdf_quad AS t0 INDEXED BY rdf_psog").is_none(), "partial index");
        assert!(parse("SELECT t0.s FROM main.rdf_quad AS t0 INDEXED BY rdf_gpso WHERE t0.g=?2").is_none());
        assert!(parse("SELECT * FROM x").is_none());
    }
}
