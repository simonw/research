//! Property path evaluation, transcribed from SPARQL 1.1 §18.4 (Property Path Patterns).
//!
//! [`PathEval::pairs`] returns the solutions of `Path(X, path, Y)` as a bag of `(x, y)` pairs,
//! where each endpoint is either a fixed term ([`End::Fixed`]: a constant, or a slot seeded by
//! an EXISTS substitution) or a variable ([`End::Free`]).
//!
//! * A link `:p`, `^path`, `path1/path2` and `path1|path2` follow the algebra literally: a
//!   sequence is a join on a fresh middle variable followed by a projection, an alternative is
//!   a union. Both keep duplicates. The middle of a sequence is a *variable*: each operand is
//!   evaluated with it free, so its zero-length matches range over `nodes(G)` and never fix it
//!   to an endpoint constant that is not a node (`:zz (:p?/:q?) ?y` has no solution).
//! * `path?`, `path*` and `path+` produce sets. `*` and `+` use the ALP procedure: a
//!   depth-first walk with a visited set, which terminates on cycles.
//! * Zero-length matches: a fixed endpoint matches itself even when it is not a node of the
//!   graph (`:o :p* ?x` binds `?x = :o`); with both endpoints variables, every node of the
//!   active graph (`nodes(G)`: subjects and objects) matches itself.
//! * A negated property set matches the (x, y) pairs linked by at least one predicate outside
//!   the set: a set, because §18.4 defines it with an existential over the triples.
use std::cell::RefCell;
use std::ptr;
use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHashSet};
use sq_core::ir::PathPlan;
use sq_core::{Result, TermId};

use crate::dataset::{Graph, GraphReader};

/// An endpoint of a path pattern.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum End {
    Fixed(TermId),
    Free,
}

impl End {
    fn fixed(self) -> Option<TermId> {
        match self {
            End::Fixed(t) => Some(t),
            End::Free => None,
        }
    }

    fn admits(self, t: TermId) -> bool {
        self.fixed().is_none_or(|f| f == t)
    }
}

/// Direction of an ALP walk: along the path, or along its inverse.
#[derive(Copy, Clone)]
enum Dir {
    Forward,
    Backward,
}

/// Which endpoint a [`Side`]-indexed operand is keyed on.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
enum Side {
    Start,
    End,
}

/// The pairs of a path operand grouped by one endpoint: key -> the other endpoint, one entry
/// per pair (duplicates kept).
type Index = FxHashMap<TermId, Vec<TermId>>;

/// A sequence operand evaluated once: (operand, its endpoints, the endpoint it is keyed on).
/// The operand is identified by address; it is borrowed for the evaluator's lifetime.
type OperandKey = (*const PathPlan, End, End, Side);

/// Evaluates one property path pattern over one graph of the dataset.
pub(crate) struct PathEval<'a> {
    reader: GraphReader<'a>,
    graph: Graph<'a>,
    /// Sequence operands evaluated with their middle free, indexed on the middle. Inside a
    /// closure the same operand is joined once per visited node; it is evaluated only once.
    operands: RefCell<FxHashMap<OperandKey, Rc<Index>>>,
}

impl<'a> PathEval<'a> {
    pub(crate) fn new(reader: GraphReader<'a>, graph: Graph<'a>) -> Self {
        PathEval { reader, graph, operands: RefCell::default() }
    }

    /// eval(Path(from, path, to)) as a bag of (x, y) pairs.
    pub(crate) fn pairs(&self, path: &'a PathPlan, from: End, to: End) -> Result<Vec<(TermId, TermId)>> {
        Ok(match path {
            PathPlan::Pred(p) => {
                let mut triples = Vec::new();
                self.reader.triples(self.graph, from.fixed(), Some(*p), to.fixed(), &mut triples)?;
                triples.into_iter().map(|[s, _, o]| (s, o)).collect()
            }
            PathPlan::Nps(excluded) => {
                // { μ | ∃ triple (x, p, y) in G with p ∉ S }: one pair however many predicates
                // connect x and y.
                let mut triples = Vec::new();
                self.reader.triples(self.graph, from.fixed(), None, to.fixed(), &mut triples)?;
                dedup_in_order(
                    triples.into_iter().filter(|[_, p, _]| !excluded.contains(p)).map(|[s, _, o]| (s, o)).collect(),
                )
            }
            PathPlan::Inv(inner) => self.pairs(inner, to, from)?.into_iter().map(|(x, y)| (y, x)).collect(),
            PathPlan::Seq(first, second) => {
                // A = Join(eval(Path(X, first, V)), eval(Path(V, second, Y))) for a fresh
                // variable V, then Project(A, Var(X, Y)), duplicates kept. V is free in BOTH
                // operands: binding it to the other operand's value instead would turn it into
                // a constant, and a constant matches itself through a zero-length step even
                // when it is not in nodes(G). The operand on a fixed endpoint's side drives the
                // join; the other one is evaluated once and indexed on V.
                let mut out = Vec::new();
                if from.fixed().is_some() || to.fixed().is_none() {
                    let right = self.operand(second, End::Free, to, Side::Start)?;
                    for (x, v) in self.pairs(first, from, End::Free)? {
                        if let Some(ys) = right.get(&v) {
                            out.extend(ys.iter().map(|&y| (x, y)));
                        }
                    }
                } else {
                    let left = self.operand(first, from, End::Free, Side::End)?;
                    for (v, y) in self.pairs(second, End::Free, to)? {
                        if let Some(xs) = left.get(&v) {
                            out.extend(xs.iter().map(|&x| (x, y)));
                        }
                    }
                }
                out
            }
            PathPlan::Alt(a, b) => {
                let mut out = self.pairs(a, from, to)?;
                out.extend(self.pairs(b, from, to)?);
                out
            }
            PathPlan::ZeroOrOne(inner) => {
                let mut out = self.zero_length(from, to)?;
                out.extend(self.pairs(inner, from, to)?);
                dedup_in_order(out)
            }
            PathPlan::ZeroOrMore(inner) => self.closure(inner, from, to, true)?,
            PathPlan::OneOrMore(inner) => self.closure(inner, from, to, false)?,
        })
    }

    /// eval(Path(from, path, to)) indexed on the endpoint `key`, memoized for this pattern.
    fn operand(&self, path: &'a PathPlan, from: End, to: End, key: Side) -> Result<Rc<Index>> {
        let memo_key = (ptr::from_ref(path), from, to, key);
        if let Some(index) = self.operands.borrow().get(&memo_key) {
            return Ok(Rc::clone(index));
        }
        let mut index = Index::default();
        for (x, y) in self.pairs(path, from, to)? {
            let (k, other) = match key {
                Side::Start => (x, y),
                Side::End => (y, x),
            };
            index.entry(k).or_default().push(other);
        }
        let index = Rc::new(index);
        self.operands.borrow_mut().insert(memo_key, Rc::clone(&index));
        Ok(index)
    }

    /// eval(Path(X, ZeroLengthPath, Y)).
    fn zero_length(&self, from: End, to: End) -> Result<Vec<(TermId, TermId)>> {
        Ok(match (from, to) {
            (End::Fixed(x), End::Fixed(y)) => {
                if x == y {
                    vec![(x, y)]
                } else {
                    Vec::new()
                }
            }
            (End::Fixed(t), End::Free) | (End::Free, End::Fixed(t)) => vec![(t, t)],
            (End::Free, End::Free) => self.reader.nodes(self.graph)?.into_iter().map(|n| (n, n)).collect(),
        })
    }

    /// `path*` (`reflexive`) or `path+` as a set of pairs (§18.4 ZeroOrMorePath / OneOrMorePath).
    fn closure(&self, path: &'a PathPlan, from: End, to: End, reflexive: bool) -> Result<Vec<(TermId, TermId)>> {
        let mut out = Vec::new();
        let mut steps = Steps::default();
        match (from, to) {
            (End::Fixed(x), _) => {
                for y in self.alp(path, x, reflexive, Dir::Forward, &mut steps)? {
                    if to.admits(y) {
                        out.push((x, y));
                    }
                }
            }
            // eval(Path(?x, path*, y)) = eval(Path(y, (^path)*, ?x)).
            (End::Free, End::Fixed(y)) => {
                for x in self.alp(path, y, reflexive, Dir::Backward, &mut steps)? {
                    out.push((x, y));
                }
            }
            (End::Free, End::Free) => {
                for t in self.reader.nodes(self.graph)? {
                    for y in self.alp(path, t, reflexive, Dir::Forward, &mut steps)? {
                        out.push((t, y));
                    }
                }
            }
        }
        Ok(out)
    }

    /// The nodes reachable from `start` by one or more steps of `path` (zero or more when
    /// `reflexive`), each once, in visit order: the ALP procedure of §18.4 with an explicit
    /// stack instead of recursion.
    fn alp(&self, path: &'a PathPlan, start: TermId, reflexive: bool, dir: Dir, steps: &mut Steps) -> Result<Vec<TermId>> {
        let mut visited: FxHashSet<TermId> = FxHashSet::default();
        let mut order = Vec::new();
        let mut stack: Vec<TermId> =
            if reflexive { vec![start] } else { self.step(path, start, dir, steps)?.iter().rev().copied().collect() };
        while let Some(n) = stack.pop() {
            if !visited.insert(n) {
                continue;
            }
            order.push(n);
            let next = self.step(path, n, dir, steps)?;
            stack.extend(next.iter().rev().filter(|m| !visited.contains(m)));
        }
        Ok(order)
    }

    /// eval(n, path): the nodes one `path` step away from `n`, computed once per node for
    /// one closure evaluation.
    fn step<'s>(&self, path: &'a PathPlan, n: TermId, dir: Dir, steps: &'s mut Steps) -> Result<&'s [TermId]> {
        if !steps.contains_key(&n) {
            let next = match dir {
                Dir::Forward => self.pairs(path, End::Fixed(n), End::Free)?.into_iter().map(|(_, y)| y).collect(),
                Dir::Backward => self.pairs(path, End::Free, End::Fixed(n))?.into_iter().map(|(x, _)| x).collect(),
            };
            steps.insert(n, next);
        }
        Ok(&steps[&n])
    }
}

/// Memoized one-step successors (or predecessors) of nodes during one closure evaluation.
type Steps = FxHashMap<TermId, Vec<TermId>>;

/// Set semantics: the first occurrence of each pair, in order.
fn dedup_in_order(pairs: Vec<(TermId, TermId)>) -> Vec<(TermId, TermId)> {
    let mut seen = FxHashSet::default();
    pairs.into_iter().filter(|p| seen.insert(*p)).collect()
}
