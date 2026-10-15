//! Property path evaluation, transcribed from SPARQL 1.1 §18.4 / §18.6.
//!
//! [`PathEval::pairs`] returns the solutions of `Path(X, path, Y)` as a bag of `(x, y)` pairs,
//! where each endpoint is either a fixed term ([`End::Fixed`]: a constant, or a slot seeded by
//! an EXISTS substitution) or a variable ([`End::Free`]).
//!
//! * A link `:p`, `^path`, `path1/path2` and `path1|path2` follow the algebra literally: a
//!   sequence is a join on a fresh middle variable followed by a projection, an alternative is
//!   a union. Both keep duplicates.
//! * `path?`, `path*` and `path+` produce sets. `*` and `+` use the ALP procedure: a
//!   depth-first walk with a visited set, which terminates on cycles.
//! * Zero-length matches: a fixed endpoint matches itself even when it is not a node of the
//!   graph (`:o :p* ?x` binds `?x = :o`); with both endpoints variables, every node of the
//!   active graph (`nodes(G)`: subjects and objects) matches itself.
//! * A negated property set matches every triple whose predicate is outside the set.
use rustc_hash::{FxHashMap, FxHashSet};
use sq_core::ir::PathPlan;
use sq_core::{Result, TermId};

use crate::dataset::{Graph, GraphReader};

/// An endpoint of a path pattern.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
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

pub(crate) struct PathEval<'a> {
    pub(crate) reader: GraphReader<'a>,
    pub(crate) graph: Graph<'a>,
}

impl PathEval<'_> {
    /// eval(Path(from, path, to)) as a bag of (x, y) pairs.
    pub(crate) fn pairs(&self, path: &PathPlan, from: End, to: End) -> Result<Vec<(TermId, TermId)>> {
        Ok(match path {
            PathPlan::Pred(p) => {
                let mut triples = Vec::new();
                self.reader.triples(self.graph, from.fixed(), Some(*p), to.fixed(), &mut triples)?;
                triples.into_iter().map(|[s, _, o]| (s, o)).collect()
            }
            PathPlan::Nps(excluded) => {
                let mut triples = Vec::new();
                self.reader.triples(self.graph, from.fixed(), None, to.fixed(), &mut triples)?;
                triples.into_iter().filter(|[_, p, _]| !excluded.contains(p)).map(|[s, _, o]| (s, o)).collect()
            }
            PathPlan::Inv(inner) => self.pairs(inner, to, from)?.into_iter().map(|(x, y)| (y, x)).collect(),
            PathPlan::Seq(first, second) => {
                // Join(Path(X, first, ?v), Path(?v, second, Y)), projected onto X and Y.
                let left = self.pairs(first, from, End::Free)?;
                let mut ends: FxHashMap<TermId, Vec<TermId>> = FxHashMap::default();
                let mut out = Vec::new();
                for (x, middle) in left {
                    if !ends.contains_key(&middle) {
                        let right = self.pairs(second, End::Fixed(middle), to)?;
                        ends.insert(middle, right.into_iter().map(|(_, y)| y).collect());
                    }
                    out.extend(ends[&middle].iter().map(|y| (x, *y)));
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

    /// `path*` (`reflexive`) or `path+` as a set of pairs (§18.6 ZeroOrMorePath/OneOrMorePath).
    fn closure(&self, path: &PathPlan, from: End, to: End, reflexive: bool) -> Result<Vec<(TermId, TermId)>> {
        let mut out = Vec::new();
        match (from, to) {
            (End::Fixed(x), _) => {
                for y in self.alp(path, x, reflexive, Dir::Forward)? {
                    if to.admits(y) {
                        out.push((x, y));
                    }
                }
            }
            // eval(Path(?x, path*, y)) = eval(Path(y, (^path)*, ?x)).
            (End::Free, End::Fixed(y)) => {
                for x in self.alp(path, y, reflexive, Dir::Backward)? {
                    out.push((x, y));
                }
            }
            (End::Free, End::Free) => {
                for t in self.reader.nodes(self.graph)? {
                    for y in self.alp(path, t, reflexive, Dir::Forward)? {
                        out.push((t, y));
                    }
                }
            }
        }
        Ok(out)
    }

    /// The nodes reachable from `start` by one or more steps of `path` (zero or more when
    /// `reflexive`), each once, in visit order: the ALP procedure of §18.6 with an explicit
    /// stack instead of recursion.
    fn alp(&self, path: &PathPlan, start: TermId, reflexive: bool, dir: Dir) -> Result<Vec<TermId>> {
        let mut visited: FxHashSet<TermId> = FxHashSet::default();
        let mut order = Vec::new();
        let mut stack = if reflexive { vec![start] } else { self.step(path, start, dir)? };
        stack.reverse();
        while let Some(n) = stack.pop() {
            if !visited.insert(n) {
                continue;
            }
            order.push(n);
            let mut next = self.step(path, n, dir)?;
            next.reverse();
            stack.extend(next.into_iter().filter(|m| !visited.contains(m)));
        }
        Ok(order)
    }

    /// eval(n, path): the nodes one `path` step away from `n`.
    fn step(&self, path: &PathPlan, n: TermId, dir: Dir) -> Result<Vec<TermId>> {
        Ok(match dir {
            Dir::Forward => self.pairs(path, End::Fixed(n), End::Free)?.into_iter().map(|(_, y)| y).collect(),
            Dir::Backward => self.pairs(path, End::Free, End::Fixed(n))?.into_iter().map(|(x, _)| x).collect(),
        })
    }
}

/// Set semantics: the first occurrence of each pair, in order.
fn dedup_in_order(pairs: Vec<(TermId, TermId)>) -> Vec<(TermId, TermId)> {
    let mut seen = FxHashSet::default();
    pairs.into_iter().filter(|p| seen.insert(*p)).collect()
}
