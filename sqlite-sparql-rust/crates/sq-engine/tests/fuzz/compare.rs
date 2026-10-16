//! Query outcomes and their comparison: multisets, sets or sequences of rows, equal modulo a
//! bijection between "free" blank nodes (blank nodes minted during evaluation, whose labels
//! are arbitrary). Store blank nodes have the same labels in every engine reading the same
//! store and must match exactly.
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

use sq_core::Term;

use super::query::Order;

pub type Row = Vec<Option<Arc<Term>>>;

#[derive(Clone, Debug)]
pub enum Outcome {
    Solutions { vars: Vec<String>, rows: Vec<Row> },
    Boolean(bool),
    /// CONSTRUCT / DESCRIBE triples (a set).
    Graph(Vec<Row>),
    Error(String),
}

impl Outcome {
    pub fn size(&self) -> usize {
        match self {
            Outcome::Solutions { rows, .. } | Outcome::Graph(rows) => rows.len(),
            Outcome::Boolean(_) | Outcome::Error(_) => 1,
        }
    }

    /// A readable dump (rows sorted unless `order` is a sequence), for repro reports.
    pub fn describe(&self, order: Order, max_rows: usize) -> String {
        match self {
            Outcome::Error(e) => format!("ERROR {e}\n"),
            Outcome::Boolean(b) => format!("{b}\n"),
            Outcome::Solutions { vars, rows } => {
                let mut out = format!("vars: {}\n", vars.iter().map(|v| format!("?{v}")).collect::<Vec<_>>().join(" "));
                out.push_str(&show_rows(rows, order, max_rows));
                out
            }
            Outcome::Graph(rows) => show_rows(rows, Order::Set, max_rows),
        }
    }
}

fn show_rows(rows: &[Row], order: Order, max_rows: usize) -> String {
    let mut lines: Vec<String> = rows.iter().map(|r| row_text(r, &|_| false)).collect();
    if order != Order::Sequence {
        lines.sort();
    }
    let mut out = String::new();
    for l in lines.iter().take(max_rows) {
        let _ = writeln!(out, "  {l}");
    }
    if lines.len() > max_rows {
        let _ = writeln!(out, "  ... {} rows in all", lines.len());
    }
    if lines.is_empty() {
        out.push_str("  (no rows)\n");
    }
    out
}

fn term_text(t: &Option<Arc<Term>>, free: &dyn Fn(&str) -> bool) -> String {
    match t.as_deref() {
        None => "UNDEF".to_owned(),
        Some(Term::BNode(l)) if free(l) => "_:*".to_owned(),
        Some(t) => t.to_ntriples().replace("http://ex/", ":"),
    }
}

fn row_text(r: &Row, free: &dyn Fn(&str) -> bool) -> String {
    r.iter().map(|t| term_text(t, free)).collect::<Vec<_>>().join(" ")
}

/// Which blank nodes may be renamed consistently.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Free {
    /// Only blank nodes minted during evaluation (TEMP labels `t<n>`); store blank nodes
    /// (`b<hex>`) are compared exactly.
    Minted,
    /// Every blank node (comparing results computed on two different stores).
    All,
}

impl Free {
    fn test(self) -> impl Fn(&str) -> bool {
        move |label: &str| match self {
            Free::All => true,
            Free::Minted => label.starts_with('t'),
        }
    }
}

/// Equal outcomes? `Err` explains the first difference.
pub fn same(expected: &Outcome, got: &Outcome, order: Order, free: Free) -> Result<(), String> {
    let is_free = free.test();
    match (expected, got) {
        (Outcome::Error(_), Outcome::Error(_)) => Ok(()),
        (Outcome::Boolean(a), Outcome::Boolean(b)) if a == b => Ok(()),
        (Outcome::Solutions { vars: va, rows: a }, Outcome::Solutions { vars: vb, rows: b }) => {
            if va != vb {
                return Err(format!("variables differ: {va:?} vs {vb:?}"));
            }
            rows_equal(a, b, order, &is_free)
        }
        // CONSTRUCT / DESCRIBE output is a set: a repeated triple is a bug on either side.
        (Outcome::Graph(a), Outcome::Graph(b)) => {
            duplicate_free(a, "expected")?;
            duplicate_free(b, "actual")?;
            bag_equal(a, b, &is_free)
        }
        _ => Err("outcomes differ in kind or value".to_owned()),
    }
}

fn rows_equal(a: &[Row], b: &[Row], order: Order, free: &dyn Fn(&str) -> bool) -> Result<(), String> {
    match order {
        Order::Sequence => sequence_equal(a, b, free),
        Order::Bag => bag_equal(a, b, free),
        // REDUCED may drop duplicates but never adds any: `b` has the rows of `a` (the
        // unreduced multiset), each at most as often.
        Order::Set => {
            bag_equal(&dedup(a), &dedup(b), free)?;
            at_most_as_often(a, b, free)
        }
    }
}

/// Every row signature (free blank nodes erased) of `b` occurs at most as often as in `a`.
fn at_most_as_often(a: &[Row], b: &[Row], free: &dyn Fn(&str) -> bool) -> Result<(), String> {
    let mut count: HashMap<String, i64> = HashMap::new();
    for r in a {
        *count.entry(row_text(r, free)).or_default() += 1;
    }
    for r in b {
        let n = count.entry(row_text(r, free)).or_default();
        *n -= 1;
        if *n < 0 {
            return Err(format!("[{}] occurs more often than in the unreduced answer", row_text(r, free)));
        }
    }
    Ok(())
}

/// No row occurs twice (labels compared exactly).
fn duplicate_free(rows: &[Row], side: &str) -> Result<(), String> {
    let mut lines: Vec<String> = rows.iter().map(|r| row_text(r, &|_| false)).collect();
    lines.sort_unstable();
    match lines.windows(2).find(|w| w[0] == w[1]) {
        Some(w) => Err(format!("the {side} graph repeats the triple [{}]", w[0])),
        None => Ok(()),
    }
}

fn dedup(rows: &[Row]) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::with_capacity(rows.len());
    for r in rows {
        if !out.contains(r) {
            out.push(r.clone());
        }
    }
    out
}

fn has_free(r: &Row, free: &dyn Fn(&str) -> bool) -> bool {
    r.iter().any(|t| matches!(t.as_deref(), Some(Term::BNode(l)) if free(l)))
}

/// A partial blank-node bijection with an undo log.
#[derive(Default)]
struct Bijection {
    fwd: HashMap<Arc<str>, Arc<str>>,
    bwd: HashMap<Arc<str>, Arc<str>>,
    log: Vec<Arc<str>>,
}

impl Bijection {
    /// Extends the mapping so that row `x` maps onto row `y`; false (and nothing changed) if
    /// impossible.
    fn unify(&mut self, x: &Row, y: &Row, free: &dyn Fn(&str) -> bool) -> bool {
        let mark = self.log.len();
        for (s, t) in x.iter().zip(y) {
            let ok = match (s.as_deref(), t.as_deref()) {
                (Some(Term::BNode(l)), Some(Term::BNode(m))) if free(l) && free(m) => {
                    match (self.fwd.get(l), self.bwd.get(m)) {
                        (Some(f), _) => f == m,
                        (None, Some(_)) => false,
                        (None, None) => {
                            self.fwd.insert(l.clone(), m.clone());
                            self.bwd.insert(m.clone(), l.clone());
                            self.log.push(l.clone());
                            true
                        }
                    }
                }
                (s, t) => s == t,
            };
            if !ok {
                self.undo(mark);
                return false;
            }
        }
        true
    }

    fn undo(&mut self, mark: usize) {
        while self.log.len() > mark {
            let l = self.log.pop().unwrap();
            if let Some(m) = self.fwd.remove(&l) {
                self.bwd.remove(&m);
            }
        }
    }
}

fn sequence_equal(a: &[Row], b: &[Row], free: &dyn Fn(&str) -> bool) -> Result<(), String> {
    if a.len() != b.len() {
        return Err(format!("expected {} rows, got {}", a.len(), b.len()));
    }
    let mut bij = Bijection::default();
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        if !bij.unify(x, y, free) {
            return Err(format!("row {i} differs: expected [{}], got [{}]", row_text(x, &|_| false), row_text(y, &|_| false)));
        }
    }
    Ok(())
}

/// Search budget for the blank-node bijection (unify attempts).
const BIJECTION_BUDGET: usize = 200_000;

fn bag_equal(a: &[Row], b: &[Row], free: &dyn Fn(&str) -> bool) -> Result<(), String> {
    if a.len() != b.len() {
        return Err(format!("expected {} rows, got {}{}", a.len(), b.len(), multiset_diff(a, b, free)));
    }
    // Signatures (free blank nodes erased) must agree as multisets.
    let sig = |r: &Row| row_text(r, free);
    let mut sa: Vec<String> = a.iter().map(sig).collect();
    let mut sb: Vec<String> = b.iter().map(sig).collect();
    sa.sort();
    sb.sort();
    if sa != sb {
        return Err(format!("rows differ{}", multiset_diff(a, b, free)));
    }
    // Rows without free blank nodes are equal as multisets now; match the others.
    let xa: Vec<&Row> = a.iter().filter(|r| has_free(r, free)).collect();
    let xb: Vec<&Row> = b.iter().filter(|r| has_free(r, free)).collect();
    if xa.is_empty() {
        return Ok(());
    }
    let mut by_sig: HashMap<String, Vec<usize>> = HashMap::new();
    for (j, r) in xb.iter().enumerate() {
        by_sig.entry(sig(r)).or_default().push(j);
    }
    // Most constrained rows first.
    let mut order: Vec<usize> = (0..xa.len()).collect();
    order.sort_by_key(|i| by_sig[&sig(xa[*i])].len());
    let candidates: Vec<&Vec<usize>> = order.iter().map(|i| &by_sig[&sig(xa[*i])]).collect();
    let mut bij = Bijection::default();
    match search(&xa, &xb, &order, &candidates, &mut bij, free) {
        Some(true) => Ok(()),
        Some(false) => Err("no blank-node bijection maps the expected rows onto the actual rows".to_owned()),
        // Undecided: a failure, so that a hard mismatch is never waved through.
        None => Err(format!("no blank-node bijection found within {BIJECTION_BUDGET} unification attempts")),
    }
}

/// Backtracking search for a matching of the rows under one blank-node bijection, with an
/// explicit stack (answers can have thousands of rows). Row `xa[order[k]]` is placed on one of
/// `candidates[k]` (the rows of `xb` with its signature). None = budget exhausted.
fn search(
    xa: &[&Row],
    xb: &[&Row],
    order: &[usize],
    candidates: &[&Vec<usize>],
    bij: &mut Bijection,
    free: &dyn Fn(&str) -> bool,
) -> Option<bool> {
    // For each placed row: the candidate it took and the bijection log mark before it.
    let mut choice: Vec<(usize, usize)> = Vec::with_capacity(order.len());
    let mut used = vec![false; xb.len()];
    let mut budget = BIJECTION_BUDGET;
    let mut start = 0;
    loop {
        let k = choice.len();
        if k == order.len() {
            return Some(true);
        }
        let x = xa[order[k]];
        let mark = bij.log.len();
        let mut found = None;
        for (c, &j) in candidates[k].iter().enumerate().skip(start) {
            if used[j] {
                continue;
            }
            if budget == 0 {
                return None;
            }
            budget -= 1;
            if bij.unify(x, xb[j], free) {
                found = Some(c);
                break;
            }
        }
        match found {
            Some(c) => {
                used[candidates[k][c]] = true;
                choice.push((c, mark));
                start = 0;
            }
            None => {
                // Undo the previous row's choice and try its next candidate.
                let Some((c, mark)) = choice.pop() else {
                    return Some(false);
                };
                used[candidates[choice.len()][c]] = false;
                bij.undo(mark);
                start = c + 1;
            }
        }
    }
}

/// `; missing: .. ; unexpected: ..` (by signature, a few of each).
fn multiset_diff(a: &[Row], b: &[Row], free: &dyn Fn(&str) -> bool) -> String {
    let mut count: HashMap<String, i64> = HashMap::new();
    for r in a {
        *count.entry(row_text(r, free)).or_default() += 1;
    }
    for r in b {
        *count.entry(row_text(r, free)).or_default() -= 1;
    }
    let mut missing: Vec<String> = count.iter().filter(|(_, n)| **n > 0).map(|(r, n)| format!("[{r}] x{n}")).collect();
    let mut extra: Vec<String> = count.iter().filter(|(_, n)| **n < 0).map(|(r, n)| format!("[{r}] x{}", -n)).collect();
    missing.sort();
    extra.sort();
    let mut out = String::new();
    if !missing.is_empty() {
        let _ = write!(out, "; missing: {}", missing.iter().take(6).cloned().collect::<Vec<_>>().join(", "));
    }
    if !extra.is_empty() {
        let _ = write!(out, "; unexpected: {}", extra.iter().take(6).cloned().collect::<Vec<_>>().join(", "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(l: &str) -> Option<Arc<Term>> {
        Some(Arc::new(Term::BNode(l.into())))
    }

    fn i(s: &str) -> Option<Arc<Term>> {
        Some(Arc::new(Term::Iri(s.into())))
    }

    fn sols(rows: Vec<Row>) -> Outcome {
        Outcome::Solutions { vars: vec!["x".into(), "y".into()], rows }
    }

    #[test]
    fn minted_blank_nodes_match_under_a_bijection() {
        let a = sols(vec![vec![b("t1"), i("a")], vec![b("t2"), i("b")], vec![b("t1"), i("c")]]);
        let ok = sols(vec![vec![b("t9"), i("b")], vec![b("t7"), i("a")], vec![b("t7"), i("c")]]);
        let bad = sols(vec![vec![b("t9"), i("b")], vec![b("t7"), i("a")], vec![b("t9"), i("c")]]);
        assert!(same(&a, &ok, Order::Bag, Free::Minted).is_ok());
        assert!(same(&a, &bad, Order::Bag, Free::Minted).is_err());
        // Store blank nodes must match exactly unless every blank node is free.
        let s1 = sols(vec![vec![b("b1"), None]]);
        let s2 = sols(vec![vec![b("b2"), None]]);
        assert!(same(&s1, &s2, Order::Bag, Free::Minted).is_err());
        assert!(same(&s1, &s2, Order::Bag, Free::All).is_ok());
    }

    #[test]
    fn backtracking_and_large_answers() {
        // (t1 a) can go to either (t8 a) or (t9 a); only t1 -> t9 lets (t1 b) match.
        let a = sols(vec![vec![b("t1"), i("a")], vec![b("t2"), i("a")], vec![b("t1"), i("b")]]);
        let ok = sols(vec![vec![b("t8"), i("a")], vec![b("t9"), i("a")], vec![b("t9"), i("b")]]);
        assert!(same(&a, &ok, Order::Bag, Free::Minted).is_ok());
        let bad = sols(vec![vec![b("t8"), i("a")], vec![b("t9"), i("a")], vec![b("t7"), i("b")]]);
        assert!(same(&a, &bad, Order::Bag, Free::Minted).is_err());
        // Thousands of rows with blank nodes: no recursion depth problem.
        let rows = |p: &str| (0..20_000).map(|n| vec![b(&format!("{p}{n}")), i("x")]).collect::<Vec<_>>();
        assert!(same(&sols(rows("t")), &sols(rows("t")), Order::Bag, Free::Minted).is_ok());
    }

    #[test]
    fn bags_sets_and_sequences() {
        let a = sols(vec![vec![i("a"), None], vec![i("a"), None], vec![i("b"), None]]);
        let b2 = sols(vec![vec![i("b"), None], vec![i("a"), None], vec![i("a"), None]]);
        let set = sols(vec![vec![i("b"), None], vec![i("a"), None]]);
        assert!(same(&a, &b2, Order::Bag, Free::Minted).is_ok());
        assert!(same(&a, &b2, Order::Sequence, Free::Minted).is_err());
        assert!(same(&a, &set, Order::Bag, Free::Minted).is_err());
        assert!(same(&a, &set, Order::Set, Free::Minted).is_ok());
        assert!(same(&a, &a, Order::Set, Free::Minted).is_ok());
        // REDUCED never adds duplicates.
        let more = sols(vec![vec![i("a"), None], vec![i("b"), None], vec![i("b"), None]]);
        assert!(same(&a, &more, Order::Set, Free::Minted).is_err());
        // Graph outputs are sets on both sides.
        let g = |rows: Vec<Row>| Outcome::Graph(rows);
        let t = || vec![i("s"), i("p"), b("t1")];
        let u = || vec![i("s"), i("p"), b("t2")];
        assert!(same(&g(vec![t(), u()]), &g(vec![u(), t()]), Order::Bag, Free::Minted).is_ok());
        assert!(same(&g(vec![t(), u()]), &g(vec![t(), t()]), Order::Bag, Free::Minted).is_err());
        assert!(same(&g(vec![t()]), &g(vec![t(), t()]), Order::Bag, Free::Minted).is_err());
        assert!(same(&a, &Outcome::Boolean(true), Order::Bag, Free::Minted).is_err());
        assert!(same(&Outcome::Error("x".into()), &Outcome::Error("y".into()), Order::Bag, Free::Minted).is_ok());
    }
}
