//! SELECT result comparison (§12.2): variable sets, then the rows as a multiset under ONE
//! blank-node bijection for the whole result set, optionally constrained by row order.
//!
//! Ground rows (no blank node) are compared by counting. Rows with blank nodes are matched by
//! backtracking over a bijection between the two sides' blank nodes. The search is pruned by
//! colour refinement: every blank node starts with one colour, and each round recolours it
//! by the multiset of its occurrences (row group, column, row shape with the neighbours'
//! colours and the row's repetition pattern). Only rows of identical coloured shape can
//! match, and only identically coloured blank nodes can map to each other.
use std::collections::{BTreeSet, HashMap};

use oxrdf::{Term, Variable};

use super::numeric::Numeric;
use super::ties::order_keys_tie;
use super::RowOrder;
use crate::results::{format_row, Solutions};

type Row = Vec<Option<Term>>;

/// Search steps after which the bijection search gives up (reported as a failure).
const SEARCH_BUDGET: u64 = 2_000_000;
/// Colour refinement stops after this many rounds even if it still splits colours.
const MAX_REFINEMENT_ROUNDS: usize = 16;
/// Rows listed in a failure message, per side.
const MAX_DIFF_ROWS: usize = 12;

/// Compares two SELECT results. `order` applies only when `expected.ordered`; `lax`
/// (`mf:LaxCardinality`) compares the rows as sets.
pub fn compare_solutions(
    expected: &Solutions,
    actual: &Solutions,
    order: &RowOrder,
    lax: bool,
    numeric: Numeric,
) -> Result<(), String> {
    let expected_vars: BTreeSet<&str> = expected.variables.iter().map(Variable::as_str).collect();
    let actual_vars: BTreeSet<&str> = actual.variables.iter().map(Variable::as_str).collect();
    if expected_vars != actual_vars {
        return Err(format!("variables differ: expected {expected_vars:?}, got {actual_vars:?}"));
    }
    let variables = &expected.variables;
    let columns: Vec<usize> = variables
        .iter()
        .map(|v| actual.variables.iter().position(|x| x == v).expect("same variable sets"))
        .collect();

    let mut exp: Vec<Row> =
        expected.rows.iter().map(|r| r.iter().map(|t| t.as_ref().map(|t| numeric.apply(t))).collect()).collect();
    let mut act: Vec<Row> = actual
        .rows
        .iter()
        .map(|r| columns.iter().map(|&j| r.get(j).cloned().flatten().map(|t| numeric.apply(&t))).collect())
        .collect();
    if lax {
        dedup(&mut exp);
        dedup(&mut act);
    }

    let order = if lax || !expected.ordered { &RowOrder::Unordered } else { order };
    let exp_groups = groups(&exp, order, variables);
    let act_groups = groups(&act, order, variables);
    if exp.len() == act.len() && rows_match(&exp, &exp_groups, &act, &act_groups)? {
        return Ok(());
    }
    if *order != RowOrder::Unordered {
        let flat = vec![0; exp.len()];
        if exp.len() == act.len() && rows_match(&exp, &flat, &act, &flat)? {
            return Err(format!("the rows match but not in the required order ({})", order.describe()));
        }
    }
    Err(describe_difference(variables, &exp, &act))
}

/// Removes duplicate rows, keeping first occurrences.
fn dedup(rows: &mut Vec<Row>) {
    let mut seen = std::collections::HashSet::new();
    rows.retain(|r| seen.insert(r.clone()));
}

/// Group number of each row: rows only match rows of the same group. Unordered = one group;
/// a sequence = one group per row; runs = one group per run of tied ORDER BY keys (equal
/// under SPARQL's ordering, not necessarily the same RDF terms: see `ties`).
fn groups(rows: &[Row], order: &RowOrder, variables: &[Variable]) -> Vec<usize> {
    match order {
        RowOrder::Unordered => vec![0; rows.len()],
        RowOrder::Sequence => (0..rows.len()).collect(),
        RowOrder::Runs(keys) => {
            let Some(cols) = keys.iter().map(|k| variables.iter().position(|v| v == k)).collect::<Option<Vec<_>>>()
            else {
                // A key is not a result column: the exact sequence is compared.
                return (0..rows.len()).collect();
            };
            let mut group = 0;
            let mut out = Vec::with_capacity(rows.len());
            for (i, row) in rows.iter().enumerate() {
                if i > 0 && cols.iter().any(|&c| !order_keys_tie(rows[i - 1][c].as_ref(), row[c].as_ref())) {
                    group += 1;
                }
                out.push(group);
            }
            out
        }
    }
}

fn has_blank(row: &[Option<Term>]) -> bool {
    row.iter().any(|t| matches!(t, Some(Term::BlankNode(_))))
}

/// Do the two row lists match under one blank-node bijection, respecting groups?
fn rows_match(exp: &[Row], exp_groups: &[usize], act: &[Row], act_groups: &[usize]) -> Result<bool, String> {
    // Ground rows: multiset equality per group.
    let mut counts: HashMap<(usize, &Row), i64> = HashMap::new();
    for (row, g) in exp.iter().zip(exp_groups).filter(|(r, _)| !has_blank(r)) {
        *counts.entry((*g, row)).or_default() += 1;
    }
    for (row, g) in act.iter().zip(act_groups).filter(|(r, _)| !has_blank(r)) {
        *counts.entry((*g, row)).or_default() -= 1;
    }
    if counts.values().any(|c| *c != 0) {
        return Ok(false);
    }

    // Rows with blank nodes.
    let mut terms = TermInterner::default();
    let e = BlankTable::new(exp, exp_groups, &mut terms);
    let a = BlankTable::new(act, act_groups, &mut terms);
    if e.rows.len() != a.rows.len() || e.blank_count != a.blank_count {
        return Ok(false);
    }
    if e.rows.is_empty() {
        return Ok(true);
    }
    let [ce, ca] = refine_colours([&e, &a]);
    let mut colour_counts: HashMap<u32, i64> = HashMap::new();
    ce.iter().for_each(|c| *colour_counts.entry(*c).or_default() += 1);
    ca.iter().for_each(|c| *colour_counts.entry(*c).or_default() -= 1);
    if colour_counts.values().any(|c| *c != 0) {
        return Ok(false);
    }

    // Row classes under the final colouring; candidates are actual rows of the same class.
    let mut classes: HashMap<Shape, usize> = HashMap::new();
    let mut class_of = |t: &BlankTable, colours: &[u32], i: usize| {
        let n = classes.len();
        *classes.entry(t.shape(i, colours)).or_insert(n)
    };
    let exp_class: Vec<usize> = (0..e.rows.len()).map(|i| class_of(&e, &ce, i)).collect();
    let act_class: Vec<usize> = (0..a.rows.len()).map(|i| class_of(&a, &ca, i)).collect();
    let mut candidates: Vec<Vec<usize>> = vec![Vec::new(); classes.len()];
    for (j, c) in act_class.iter().enumerate() {
        candidates[*c].push(j);
    }
    let mut order: Vec<usize> = (0..e.rows.len()).collect();
    order.sort_by_key(|&i| (candidates[exp_class[i]].len(), i));
    if order.iter().any(|&i| candidates[exp_class[i]].is_empty()) {
        return Ok(false);
    }

    let mut search = Search {
        e: &e,
        a: &a,
        order,
        exp_class,
        candidates,
        forward: vec![None; e.blank_count],
        backward: vec![None; a.blank_count],
        used: vec![false; a.rows.len()],
        steps: 0,
    };
    search.solve(0)
}

#[derive(Default)]
struct TermInterner {
    ids: HashMap<Term, u32>,
}

impl TermInterner {
    fn id(&mut self, t: &Term) -> u32 {
        let n = self.ids.len() as u32;
        *self.ids.entry(t.clone()).or_insert(n)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum Cell {
    Unbound,
    Ground(u32),
    /// Side-local blank node index.
    Blank(u32),
}

/// A row with blank nodes replaced by (colour, column of the node's first occurrence in the
/// row): two rows match under some bijection only if their shapes are equal.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
struct Shape {
    group: usize,
    multiplicity: usize,
    cells: Vec<ShapeCell>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum ShapeCell {
    Unbound,
    Ground(u32),
    Blank { colour: u32, first: usize },
}

/// One side's distinct blank-node rows (identical rows collapsed with a multiplicity).
struct BlankTable {
    rows: Vec<(usize, Vec<Cell>, usize)>,
    blank_count: usize,
}

impl BlankTable {
    fn new(rows: &[Row], groups: &[usize], terms: &mut TermInterner) -> Self {
        let mut blanks: HashMap<&str, u32> = HashMap::new();
        let mut distinct: HashMap<(usize, Vec<Cell>), usize> = HashMap::new();
        let mut first_seen = Vec::new();
        for (row, g) in rows.iter().zip(groups).filter(|(r, _)| has_blank(r)) {
            let cells: Vec<Cell> = row
                .iter()
                .map(|t| match t {
                    None => Cell::Unbound,
                    Some(Term::BlankNode(b)) => {
                        let n = blanks.len() as u32;
                        Cell::Blank(*blanks.entry(b.as_str()).or_insert(n))
                    }
                    Some(t) => Cell::Ground(terms.id(t)),
                })
                .collect();
            let key = (*g, cells);
            match distinct.get_mut(&key) {
                Some(m) => *m += 1,
                None => {
                    first_seen.push(key.clone());
                    distinct.insert(key, 1);
                }
            }
        }
        let rows = first_seen
            .into_iter()
            .map(|key| {
                let m = distinct[&key];
                (key.0, key.1, m)
            })
            .collect();
        BlankTable { rows, blank_count: blanks.len() }
    }

    fn shape(&self, i: usize, colours: &[u32]) -> Shape {
        let (group, cells, multiplicity) = &self.rows[i];
        let cells = cells
            .iter()
            .enumerate()
            .map(|(col, c)| match *c {
                Cell::Unbound => ShapeCell::Unbound,
                Cell::Ground(t) => ShapeCell::Ground(t),
                Cell::Blank(b) => ShapeCell::Blank {
                    colour: colours[b as usize],
                    first: cells.iter().position(|x| *x == Cell::Blank(b)).unwrap_or(col),
                },
            })
            .collect();
        Shape { group: *group, multiplicity: *multiplicity, cells }
    }
}

/// Colour refinement over both sides at once (shared colour space).
fn refine_colours(sides: [&BlankTable; 2]) -> [Vec<u32>; 2] {
    let mut colours = [vec![0u32; sides[0].blank_count], vec![0u32; sides[1].blank_count]];
    let mut distinct = 1;
    for _ in 0..MAX_REFINEMENT_ROUNDS {
        let mut shapes: HashMap<Shape, u32> = HashMap::new();
        let mut signatures: HashMap<(u32, Vec<(usize, u32)>), u32> = HashMap::new();
        let mut next = [Vec::new(), Vec::new()];
        for (side, table) in sides.iter().enumerate() {
            let mut occurrences: Vec<Vec<(usize, u32)>> = vec![Vec::new(); table.blank_count];
            for i in 0..table.rows.len() {
                let shape = table.shape(i, &colours[side]);
                let n = shapes.len() as u32;
                let shape_id = *shapes.entry(shape).or_insert(n);
                for (col, c) in table.rows[i].1.iter().enumerate() {
                    if let Cell::Blank(b) = c {
                        occurrences[*b as usize].push((col, shape_id));
                    }
                }
            }
            next[side] = occurrences
                .into_iter()
                .enumerate()
                .map(|(b, mut occ)| {
                    occ.sort_unstable();
                    let n = signatures.len() as u32;
                    *signatures.entry((colours[side][b], occ)).or_insert(n)
                })
                .collect();
        }
        colours = next;
        if signatures.len() == distinct {
            break;
        }
        distinct = signatures.len();
    }
    colours
}

struct Search<'a> {
    e: &'a BlankTable,
    a: &'a BlankTable,
    order: Vec<usize>,
    exp_class: Vec<usize>,
    candidates: Vec<Vec<usize>>,
    forward: Vec<Option<u32>>,
    backward: Vec<Option<u32>>,
    used: Vec<bool>,
    steps: u64,
}

impl Search<'_> {
    fn solve(&mut self, k: usize) -> Result<bool, String> {
        let Some(&i) = self.order.get(k) else { return Ok(true) };
        for c in 0..self.candidates[self.exp_class[i]].len() {
            let j = self.candidates[self.exp_class[i]][c];
            if self.used[j] {
                continue;
            }
            self.steps += 1;
            if self.steps > SEARCH_BUDGET {
                return Err(format!("blank-node bijection search exceeded {SEARCH_BUDGET} steps"));
            }
            let mut assigned = Vec::new();
            if self.unify(i, j, &mut assigned) {
                self.used[j] = true;
                if self.solve(k + 1)? {
                    return Ok(true);
                }
                self.used[j] = false;
            }
            for (x, y) in assigned {
                self.forward[x as usize] = None;
                self.backward[y as usize] = None;
            }
        }
        Ok(false)
    }

    /// Extends the bijection so expected row `i` maps onto actual row `j`. Ground cells
    /// already agree (same class); `assigned` records new pairs for undo.
    fn unify(&mut self, i: usize, j: usize, assigned: &mut Vec<(u32, u32)>) -> bool {
        for (x, y) in self.e.rows[i].1.iter().zip(&self.a.rows[j].1) {
            let (Cell::Blank(x), Cell::Blank(y)) = (*x, *y) else { continue };
            match (self.forward[x as usize], self.backward[y as usize]) {
                (Some(m), _) if m != y => return false,
                (_, Some(m)) if m != x => return false,
                (None, None) => {
                    self.forward[x as usize] = Some(y);
                    self.backward[y as usize] = Some(x);
                    assigned.push((x, y));
                }
                _ => {}
            }
        }
        true
    }
}

/// Human-readable difference: rows only on one side, with blank-node labels masked.
fn describe_difference(variables: &[Variable], exp: &[Row], act: &[Row]) -> String {
    let mask = |r: &Row| -> Row {
        r.iter()
            .map(|t| match t {
                Some(Term::BlankNode(_)) => Some(oxrdf::BlankNode::new_unchecked("_").into()),
                t => t.clone(),
            })
            .collect()
    };
    let mut counts: HashMap<Row, i64> = HashMap::new();
    for r in exp {
        *counts.entry(mask(r)).or_default() += 1;
    }
    for r in act {
        *counts.entry(mask(r)).or_default() -= 1;
    }
    let mut missing: Vec<String> = Vec::new();
    let mut unexpected: Vec<String> = Vec::new();
    for (row, n) in &counts {
        let text = format_row(variables, row);
        let list = if *n > 0 { &mut missing } else { &mut unexpected };
        for _ in 0..n.unsigned_abs() {
            list.push(text.clone());
        }
    }
    missing.sort();
    unexpected.sort();
    let mut out = format!("expected {} rows, got {}", exp.len(), act.len());
    if missing.is_empty() && unexpected.is_empty() {
        out.push_str("; the rows agree up to blank-node labels but no consistent blank-node bijection exists");
        return out;
    }
    for (label, list) in [("missing", &missing), ("unexpected", &unexpected)] {
        if list.is_empty() {
            continue;
        }
        out.push_str(&format!("; {} {label}:", list.len()));
        for row in list.iter().take(MAX_DIFF_ROWS) {
            out.push_str("\n    ");
            out.push_str(row);
        }
        if list.len() > MAX_DIFF_ROWS {
            out.push_str(&format!("\n    … {} more", list.len() - MAX_DIFF_ROWS));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{BlankNode, Literal, NamedNode};

    fn v(names: &[&str]) -> Vec<Variable> {
        names.iter().map(|n| Variable::new_unchecked(*n)).collect()
    }
    fn iri(s: &str) -> Option<Term> {
        Some(NamedNode::new_unchecked(format!("http://ex/{s}")).into())
    }
    fn b(s: &str) -> Option<Term> {
        Some(BlankNode::new_unchecked(s).into())
    }
    fn int(i: i64) -> Option<Term> {
        Some(Literal::from(i).into())
    }
    fn sols(vars: &[&str], rows: Vec<Row>) -> Solutions {
        Solutions { variables: v(vars), rows, ordered: true }
    }
    fn cmp(e: &Solutions, a: &Solutions, order: &RowOrder) -> Result<(), String> {
        compare_solutions(e, a, order, false, Numeric::Exact)
    }

    #[test]
    fn variable_sets_must_agree() {
        let e = sols(&["x"], vec![]);
        let a = sols(&["y"], vec![]);
        assert!(cmp(&e, &a, &RowOrder::Unordered).unwrap_err().contains("variables differ"));
    }

    #[test]
    fn column_order_is_irrelevant_and_rows_are_a_multiset() {
        let e = sols(&["x", "y"], vec![vec![iri("a"), int(1)], vec![iri("a"), int(1)], vec![iri("b"), None]]);
        let a = sols(&["y", "x"], vec![vec![None, iri("b")], vec![int(1), iri("a")], vec![int(1), iri("a")]]);
        cmp(&e, &a, &RowOrder::Unordered).unwrap();
        let fewer = sols(&["y", "x"], vec![vec![None, iri("b")], vec![int(1), iri("a")]]);
        let err = cmp(&e, &fewer, &RowOrder::Unordered).unwrap_err();
        assert!(err.contains("expected 3 rows, got 2") && err.contains("1 missing"), "{err}");
    }

    #[test]
    fn blank_nodes_need_a_global_bijection() {
        // Expected: _:a appears in two rows, _:b in one.
        let e = sols(&["s", "o"], vec![vec![b("a"), int(1)], vec![b("a"), int(2)], vec![b("b"), int(3)]]);
        let ok = sols(&["s", "o"], vec![vec![b("y"), int(3)], vec![b("x"), int(2)], vec![b("x"), int(1)]]);
        cmp(&e, &ok, &RowOrder::Unordered).unwrap();
        // Same rows up to labels, but the co-reference differs.
        let bad = sols(&["s", "o"], vec![vec![b("x"), int(1)], vec![b("y"), int(2)], vec![b("y"), int(3)]]);
        let err = cmp(&e, &bad, &RowOrder::Unordered).unwrap_err();
        assert!(err.contains("no consistent blank-node bijection"), "{err}");
        // Injectivity: two expected nodes cannot map to one actual node.
        let e2 = sols(&["s"], vec![vec![b("a")], vec![b("b")]]);
        let a2 = sols(&["s"], vec![vec![b("x")], vec![b("x")]]);
        assert!(cmp(&e2, &a2, &RowOrder::Unordered).is_err());
    }

    #[test]
    fn symmetric_structures_need_backtracking() {
        // A 6-cycle vs two 3-cycles: identical local structure, not isomorphic.
        let cycle = |labels: &[&str]| -> Vec<Row> {
            (0..labels.len()).map(|i| vec![b(labels[i]), b(labels[(i + 1) % labels.len()])]).collect()
        };
        let e = sols(&["x", "y"], cycle(&["a", "b", "c", "d", "e", "f"]));
        let mut two = cycle(&["p", "q", "r"]);
        two.extend(cycle(&["s", "t", "u"]));
        assert!(cmp(&e, &sols(&["x", "y"], two), &RowOrder::Unordered).is_err());
        let mut shuffled = cycle(&["5", "3", "1", "6", "2", "4"]);
        shuffled.reverse();
        cmp(&e, &sols(&["x", "y"], shuffled), &RowOrder::Unordered).unwrap();
    }

    #[test]
    fn repeated_blank_in_one_row_is_distinguished() {
        let e = sols(&["x", "y"], vec![vec![b("a"), b("a")]]);
        let a = sols(&["x", "y"], vec![vec![b("p"), b("q")]]);
        assert!(cmp(&e, &a, &RowOrder::Unordered).is_err());
    }

    #[test]
    fn sequence_and_runs() {
        let e = sols(&["k", "v"], vec![vec![int(1), iri("a")], vec![int(1), iri("b")], vec![int(2), iri("c")]]);
        let swapped_in_run = sols(&["k", "v"], vec![vec![int(1), iri("b")], vec![int(1), iri("a")], vec![int(2), iri("c")]]);
        let runs = RowOrder::Runs(v(&["k"]));
        cmp(&e, &swapped_in_run, &runs).unwrap();
        let err = cmp(&e, &swapped_in_run, &RowOrder::Sequence).unwrap_err();
        assert!(err.contains("not in the required order"), "{err}");
        let across_runs = sols(&["k", "v"], vec![vec![int(2), iri("c")], vec![int(1), iri("a")], vec![int(1), iri("b")]]);
        assert!(cmp(&e, &across_runs, &runs).unwrap_err().contains("not in the required order"));
        // An unordered expectation (RDF result set without rs:index) ignores ORDER BY.
        let mut unordered = e.clone();
        unordered.ordered = false;
        cmp(&unordered, &across_runs, &runs).unwrap();
        // A key that is not a result column: exact sequence.
        cmp(&e, &e, &RowOrder::Runs(v(&["hidden"]))).unwrap();
        assert!(cmp(&e, &swapped_in_run, &RowOrder::Runs(v(&["hidden"]))).is_err());
    }

    #[test]
    fn runs_group_tied_keys() {
        let dec = |s: &str| -> Option<Term> {
            Some(Literal::new_typed_literal(s, NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#decimal")).into())
        };
        let runs = RowOrder::Runs(v(&["k"]));
        // 1 and 1.0 tie; 2 does not.
        let e = sols(&["k", "v"], vec![vec![int(1), iri("a")], vec![dec("1.0"), iri("b")], vec![int(2), iri("c")]]);
        let swapped = sols(&["k", "v"], vec![vec![dec("1.0"), iri("b")], vec![int(1), iri("a")], vec![int(2), iri("c")]]);
        cmp(&e, &swapped, &runs).unwrap();
        let late = sols(&["k", "v"], vec![vec![int(2), iri("c")], vec![dec("1.0"), iri("b")], vec![int(1), iri("a")]]);
        assert!(cmp(&e, &late, &runs).unwrap_err().contains("not in the required order"));
        // Blank-node keys form one run, which still sorts before IRI keys.
        let e = sols(&["k", "v"], vec![vec![b("x"), int(1)], vec![b("y"), int(2)], vec![iri("z"), int(3)]]);
        let a = sols(&["k", "v"], vec![vec![b("q"), int(2)], vec![b("p"), int(1)], vec![iri("z"), int(3)]]);
        cmp(&e, &a, &runs).unwrap();
        let a = sols(&["k", "v"], vec![vec![b("q"), int(2)], vec![iri("z"), int(3)], vec![b("p"), int(1)]]);
        assert!(cmp(&e, &a, &runs).unwrap_err().contains("not in the required order"));
        // A tie in the first key leaves the second key in force.
        let runs2 = RowOrder::Runs(v(&["k", "v"]));
        let e = sols(&["k", "v"], vec![vec![int(1), int(1)], vec![dec("1.0"), int(2)]]);
        let a = sols(&["k", "v"], vec![vec![dec("1.0"), int(2)], vec![int(1), int(1)]]);
        assert!(cmp(&e, &a, &runs2).is_err());
    }

    #[test]
    fn lax_cardinality_compares_sets() {
        let e = sols(&["x"], vec![vec![iri("a")], vec![iri("b")]]);
        let a = sols(&["x"], vec![vec![iri("a")], vec![iri("a")], vec![iri("b")]]);
        assert!(cmp(&e, &a, &RowOrder::Unordered).is_err());
        compare_solutions(&e, &a, &RowOrder::Unordered, true, Numeric::Exact).unwrap();
    }

    #[test]
    fn lenient_numerics() {
        let dbl = |s: &str| -> Option<Term> {
            Some(Literal::new_typed_literal(s, NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#double")).into())
        };
        let e = sols(&["x"], vec![vec![dbl("1.0e6")]]);
        let a = sols(&["x"], vec![vec![dbl("1.0E6")]]);
        assert!(cmp(&e, &a, &RowOrder::Unordered).is_err());
        compare_solutions(&e, &a, &RowOrder::Unordered, false, Numeric::Lenient).unwrap();
    }

    #[test]
    fn large_ground_results_are_fast() {
        let rows: Vec<Row> = (0..20_000).map(|i| vec![int(i), iri(&format!("n{}", i % 13))]).collect();
        let mut rev = rows.clone();
        rev.reverse();
        cmp(&sols(&["a", "b"], rows), &sols(&["a", "b"], rev), &RowOrder::Unordered).unwrap();
    }

    #[test]
    fn many_interchangeable_blank_rows() {
        // 300 rows, each with its own blank node and the same ground value: every expected
        // row has 300 candidates, but any assignment works (no backtracking).
        let e = sols(&["s", "o"], (0..300).map(|i| vec![b(&format!("e{i}")), int(1)]).collect());
        let a = sols(&["s", "o"], (0..300).map(|i| vec![b(&format!("a{i}")), int(1)]).collect());
        cmp(&e, &a, &RowOrder::Unordered).unwrap();
    }
}
