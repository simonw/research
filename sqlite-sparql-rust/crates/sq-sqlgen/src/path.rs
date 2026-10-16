//! Property paths → a derived table of `(s, o)` pairs.
//!
//! * `p`, `^p`, `!(…)`: scans of `main.rdf_quad`;
//! * `a/b/…` (one join), `a|b|…` (one `UNION ALL`): bag semantics, as in the spec;
//! * `p+`, `p*`: `WITH RECURSIVE … UNION` (set semantics), seeded from a constant endpoint
//!   when there is one (forward from the subject, else backward from the object);
//! * `p*`, `p?` add the zero-length pairs: a constant endpoint matches itself even when it is
//!   absent from the graph; with both endpoints free every node (subject or object) of the
//!   active graph matches itself. `x+` has them too when `x` does (its first step is a pair
//!   of `x`): `(p*)+`, `(p?)+` ([`zero_length`]).
//!
//! Inside EXISTS an endpoint substituted by the enclosing row is treated as a constant
//! (correlated reference), as the spec's substitution semantics require.
use sq_core::ir::{GraphSel, PathPlan, PatTerm};
use sq_core::{Result, TermId};

use crate::sql::{Block, Col, Item, MAX_TABLES, and_all, union_all};
use crate::translate::{Env, GraphCond, GraphCtx, Merge, QuadSrc, Translator, bind, unsupported};

/// The edges a path walks.
enum Edges {
    /// `main.rdf_quad` rows with `g = <sql>`.
    Table(String),
    /// `main.rdf_quad` rows of an RDF merge.
    Merged(Merge),
    /// An empty graph (only the zero-length pairs of constants exist).
    Nothing,
}

/// A relation of `(s, o)` pairs with no rows.
const NO_PAIRS: &str = "SELECT NULL AS s, NULL AS o WHERE 0";

/// Tables a path relation may flatten into: one less than [`MAX_TABLES`], leaving room for
/// the recursive CTE a closure step joins it with.
const PATH_TABLES: usize = MAX_TABLES - 1;

/// SQL with result columns `(s, o)`, and the tables SQLite may flatten it into.
struct Rel {
    sql: String,
    tables: usize,
}

impl Rel {
    /// A relation SQLite does not flatten into its parent's join (a compound, a CTE, …), or a
    /// single table.
    fn one(sql: String) -> Rel {
        Rel { sql, tables: 1 }
    }
}

/// Closures nested deeper than this are rejected before translation: each closure repeats
/// its operand's SQL (seed and recursive step), so the text doubles per level, and SQLite's
/// parser stack cannot take more than about four levels anyway (see `limits`).
const MAX_CLOSURE_DEPTH: usize = 6;

/// How deeply closures nest in a path.
fn closure_depth(p: &PathPlan) -> usize {
    match p {
        PathPlan::Pred(_) | PathPlan::Nps(_) => 0,
        PathPlan::Inv(x) | PathPlan::ZeroOrOne(x) => closure_depth(x),
        PathPlan::Seq(a, b) | PathPlan::Alt(a, b) => closure_depth(a).max(closure_depth(b)),
        PathPlan::OneOrMore(x) | PathPlan::ZeroOrMore(x) => 1 + closure_depth(x),
    }
}

/// The operands of a whole `a/b/…` (`seq`) or `a|b|…` chain.
fn chain<'p>(p: &'p PathPlan, seq: bool, out: &mut Vec<&'p PathPlan>) {
    match (p, seq) {
        (PathPlan::Seq(a, b), true) | (PathPlan::Alt(a, b), false) => {
            chain(a, seq, out);
            chain(b, seq, out);
        }
        _ => out.push(p),
    }
}

/// `SELECT first.s, last.o` over relations joined end to start; an `opaque` one carries
/// `LIMIT -1` so that SQLite keeps it a single table of its parent.
fn join_rels(items: &[(String, Rel)], opaque: bool) -> Rel {
    let (first, last) = (&items[0].0, &items[items.len() - 1].0);
    let from: Vec<String> = items.iter().map(|(a, r)| format!("({}) AS {a}", r.sql)).collect();
    let conds: Vec<String> = items.windows(2).map(|w| format!("{}.o = {}.s", w[0].0, w[1].0)).collect();
    let mut sql = format!("SELECT {first}.s AS s, {last}.o AS o FROM {}", from.join(", "));
    if !conds.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&and_all(&conds));
    }
    if opaque {
        sql.push_str(" LIMIT -1");
        return Rel::one(sql);
    }
    Rel { sql, tables: items.iter().map(|(_, r)| r.tables).sum() }
}

/// The FROM entry of one recursive step.
struct Step {
    from: String,
    s: String,
    o: String,
    conds: Vec<String>,
}

/// `p` → (p, false), `^p` → (p, true): a single quad lookup.
fn direct_pred(x: &PathPlan) -> Option<(TermId, bool)> {
    match x {
        PathPlan::Pred(p) => Some((*p, false)),
        PathPlan::Inv(inner) => match **inner {
            PathPlan::Pred(p) => Some((p, true)),
            _ => None,
        },
        _ => None,
    }
}

/// Can the path match a zero-length pair `(x, x)` for a node absent from the graph?
///
/// `x+` can whenever `x` can: its first step is a pair of `x`, so `(p*)+`, `(p?)+` and
/// `^(p*)+` pair every node (and every constant endpoint) with itself, exactly like `p*`.
pub(crate) fn zero_length(p: &PathPlan) -> bool {
    match p {
        PathPlan::ZeroOrMore(_) | PathPlan::ZeroOrOne(_) => true,
        PathPlan::Inv(x) | PathPlan::OneOrMore(x) => zero_length(x),
        PathPlan::Alt(a, b) => zero_length(a) || zero_length(b),
        PathPlan::Seq(a, b) => zero_length(a) && zero_length(b),
        PathPlan::Pred(_) | PathPlan::Nps(_) => false,
    }
}

impl Translator<'_> {
    /// `s path o` as a block.
    pub(crate) fn path(&mut self, g: GraphSel, s: PatTerm, path: &PathPlan, o: PatTerm, env: &Env) -> Result<Block> {
        // A plain (inverse) predicate is just a triple pattern.
        if let Some((p, inverse)) = direct_pred(path) {
            let (s, o) = if inverse { (o, s) } else { (s, o) };
            return self.block(&bgp1(g, s, p, o), env);
        }
        if closure_depth(path) > MAX_CLOSURE_DEPTH {
            return Err(unsupported(format!("property paths with closures nested more than {MAX_CLOSURE_DEPTH} deep")));
        }
        // Inside a pushed `GRAPH ?g` every solution must bind g, zero-length pairs included.
        let pushed_graph_var = g == GraphSel::Active && matches!(env.graph, GraphCtx::Var(_));
        let edges = match self.quad_source(g, env)? {
            QuadSrc::Table(GraphCond::Var(_)) => return Err(unsupported("property paths inside GRAPH ?g")),
            // No triples: only the zero-length pairs of constants remain, and with no named
            // graph at all `GRAPH ?g` has no solutions.
            QuadSrc::Nothing if pushed_graph_var || !zero_length(path) => return Ok(Block::empty()),
            QuadSrc::Nothing => Edges::Nothing,
            QuadSrc::Table(GraphCond::Eq(g)) => Edges::Table(g),
            QuadSrc::Merged(m) => Edges::Merged(m),
        };
        let zero = zero_length(path);
        let start = self.endpoint(s, env, zero)?;
        let end = self.endpoint(o, env, zero)?;
        let rel = self.path_rel(path, &edges, start.as_deref(), end.as_deref())?;
        let alias = self.alias("d");
        let mut b = Block::unit();
        b.items.push(Item::Derived { sql: rel.sql, alias: alias.clone(), tables: rel.tables });
        for (term, col) in [(s, "s"), (o, "o")] {
            if let PatTerm::Slot(x) = term {
                bind(&mut b, x, Col::new(format!("{alias}.{col}")), env);
            }
        }
        Ok(b)
    }

    /// A known endpoint value: a constant, or a slot substituted by an enclosing EXISTS row.
    fn endpoint(&mut self, t: PatTerm, env: &Env, zero: bool) -> Result<Option<String>> {
        Ok(match t {
            PatTerm::Const(c) => Some(self.param(c)),
            PatTerm::Slot(x) => match env.outer.get(&x) {
                Some(o) if !o.nullable => Some(o.sql.clone()),
                // Free in some rows and a constant in others: the zero-length pairs differ.
                Some(_) if zero => return Err(unsupported("zero-length paths with an optionally substituted endpoint")),
                _ => None,
            },
        })
    }

    /// The pairs of `p`, restricted to the known endpoints.
    fn path_rel(&mut self, p: &PathPlan, edges: &Edges, start: Option<&str>, end: Option<&str>) -> Result<Rel> {
        Ok(match p {
            PathPlan::Pred(pred) => {
                let v = self.param(*pred);
                Rel::one(self.edge_scan(edges, |e| Some(format!("{e}.p = {v}")), start, end))
            }
            PathPlan::Nps(list) => {
                let ids: Vec<String> = list.iter().map(|id| self.param(*id)).collect();
                let cond = |e: &str| (!ids.is_empty()).then(|| format!("{e}.p NOT IN ({})", ids.join(", ")));
                Rel::one(self.edge_scan(edges, cond, start, end))
            }
            PathPlan::Inv(x) => {
                let inner = self.path_rel(x, edges, end, start)?;
                let a = self.alias("d");
                Rel { sql: format!("SELECT {a}.o AS s, {a}.s AS o FROM ({}) AS {a}", inner.sql), tables: inner.tables }
            }
            PathPlan::Seq(..) => {
                let mut parts = Vec::new();
                chain(p, true, &mut parts);
                self.sequence(&parts, edges, start, end)?
            }
            PathPlan::Alt(..) => {
                let mut parts = Vec::new();
                chain(p, false, &mut parts);
                let mut terms = Vec::with_capacity(parts.len());
                let mut tables = 1;
                for part in parts {
                    let rel = self.path_rel(part, edges, start, end)?;
                    tables = tables.max(rel.tables);
                    terms.push(format!("SELECT s, o FROM ({})", rel.sql));
                }
                // SQLite may flatten a UNION ALL into its parent, one copy per branch.
                Rel { sql: union_all(terms), tables }
            }
            PathPlan::OneOrMore(x) => Rel::one(self.closure(x, edges, start, end, false)?),
            PathPlan::ZeroOrMore(x) => Rel::one(self.closure(x, edges, start, end, true)?),
            PathPlan::ZeroOrOne(x) => {
                let one = self.path_rel(x, edges, start, end)?;
                let zero = self.zero_pairs(edges, start, end);
                Rel::one(format!("SELECT s, o FROM ({}) UNION SELECT s, o FROM ({zero})", one.sql))
            }
        })
    }

    /// `a/b/…` as one join of the steps' relations (within [`PATH_TABLES`]: a prefix that
    /// would exceed it becomes one opaque relation).
    fn sequence(&mut self, parts: &[&PathPlan], edges: &Edges, start: Option<&str>, end: Option<&str>) -> Result<Rel> {
        let mut items: Vec<(String, Rel)> = Vec::with_capacity(parts.len());
        for (i, part) in parts.iter().enumerate() {
            let rel = self.path_rel(
                part,
                edges,
                if i == 0 { start } else { None },
                if i + 1 == parts.len() { end } else { None },
            )?;
            let joined: usize = items.iter().map(|(_, r)| r.tables).sum();
            if !items.is_empty() && joined + rel.tables > PATH_TABLES {
                let prefix = join_rels(&items, true);
                items = vec![(self.alias("d"), prefix)];
            }
            items.push((self.alias("d"), rel));
        }
        Ok(join_rels(&items, false))
    }

    /// The `main.rdf_quad` alias `alias` restricted to the edges (FROM entry, conditions);
    /// `once` reads each triple of a merge once (bag semantics), else copies may repeat.
    fn edge_source(&mut self, edges: &Edges, alias: &str, once: bool) -> Option<(String, Vec<String>)> {
        let conds = match edges {
            Edges::Table(g) => vec![format!("{alias}.g = {g}")],
            Edges::Merged(m) => self.merge_conds(m, alias, once),
            Edges::Nothing => return None,
        };
        Some((format!("main.rdf_quad AS {alias}"), conds))
    }

    /// `SELECT s, o` over the edges, each triple once, with an optional predicate condition
    /// (`pred(alias)`).
    fn edge_scan(
        &mut self,
        edges: &Edges,
        pred: impl FnOnce(&str) -> Option<String>,
        start: Option<&str>,
        end: Option<&str>,
    ) -> String {
        let e = self.alias("e");
        let Some((from, mut conds)) = self.edge_source(edges, &e, true) else { return NO_PAIRS.to_owned() };
        conds.extend(pred(&e));
        conds.extend(start.map(|v| format!("{e}.s = {v}")));
        conds.extend(end.map(|v| format!("{e}.o = {v}")));
        format!("SELECT {e}.s AS s, {e}.o AS o FROM {from} WHERE {}", and_all(&conds))
    }

    /// One step of a closure as a FROM entry (`main.rdf_quad` directly for `p` and `^p`, so
    /// each recursion probes an index; repeated copies of a merged triple are harmless, the
    /// closure being a set).
    fn step(&mut self, x: &PathPlan, edges: &Edges) -> Result<Step> {
        let k = self.alias("k");
        let direct = match direct_pred(x) {
            Some((p, inverse)) => self.edge_source(edges, &k, false).map(|(from, conds)| (p, inverse, from, conds)),
            None => None,
        };
        if let Some((p, inverse, from, mut conds)) = direct {
            let pv = self.param(p);
            conds.push(format!("{k}.p = {pv}"));
            let (s, o) = if inverse { ("o", "s") } else { ("s", "o") };
            return Ok(Step { from, s: format!("{k}.{s}"), o: format!("{k}.{o}"), conds });
        }
        let rel = self.path_rel(x, edges, None, None)?.sql;
        Ok(Step { from: format!("({rel}) AS {k}"), s: format!("{k}.s"), o: format!("{k}.o"), conds: Vec::new() })
    }

    /// `x+` (`reflexive = false`) or `x*` as a recursive CTE.
    fn closure(
        &mut self,
        x: &PathPlan,
        edges: &Edges,
        start: Option<&str>,
        end: Option<&str>,
        reflexive: bool,
    ) -> Result<String> {
        let step = self.step(x, edges)?;
        let r = self.alias("r");
        let and_conds = |extra: String| {
            let mut c = vec![extra];
            c.extend(step.conds.iter().cloned());
            c.join(" AND ")
        };
        Ok(match (start, end) {
            (Some(c), _) => {
                let seed = if reflexive {
                    format!("SELECT {c}")
                } else {
                    format!("SELECT o FROM ({})", self.path_rel(x, edges, Some(c), None)?.sql)
                };
                let joined = and_conds(format!("{} = {r}.n", step.s));
                let rec = format!("SELECT {} FROM {r}, {} WHERE {joined}", step.o, step.from);
                let filter = end.map(|e| format!(" WHERE n = {e}")).unwrap_or_default();
                format!("WITH RECURSIVE {r}(n) AS ({seed} UNION {rec}) SELECT {c} AS s, n AS o FROM {r}{filter}")
            }
            (None, Some(e)) => {
                let seed = if reflexive {
                    format!("SELECT {e}")
                } else {
                    format!("SELECT s FROM ({})", self.path_rel(x, edges, None, Some(e))?.sql)
                };
                let joined = and_conds(format!("{} = {r}.n", step.o));
                let rec = format!("SELECT {} FROM {r}, {} WHERE {joined}", step.s, step.from);
                format!("WITH RECURSIVE {r}(n) AS ({seed} UNION {rec}) SELECT n AS s, {e} AS o FROM {r}")
            }
            (None, None) => {
                let first = self.path_rel(x, edges, None, None)?.sql;
                let rec = format!(
                    "SELECT {r}.s, {} FROM {r}, {} WHERE {}",
                    step.o,
                    step.from,
                    and_conds(format!("{} = {r}.o", step.s))
                );
                let closure = format!(
                    "WITH RECURSIVE {r}(s, o) AS (SELECT s, o FROM ({first}) UNION {rec}) SELECT s, o FROM {r}"
                );
                if reflexive {
                    let zero = self.zero_pairs(edges, None, None);
                    format!("SELECT s, o FROM ({closure}) UNION SELECT s, o FROM ({zero})")
                } else {
                    closure
                }
            }
        })
    }

    /// The zero-length pairs `(x, x)`.
    fn zero_pairs(&mut self, edges: &Edges, start: Option<&str>, end: Option<&str>) -> String {
        match (start, end) {
            (Some(c), Some(e)) => format!("SELECT {c} AS s, {c} AS o WHERE {c} = {e}"),
            (Some(c), None) => format!("SELECT {c} AS s, {c} AS o"),
            (None, Some(e)) => format!("SELECT {e} AS s, {e} AS o"),
            (None, None) => {
                let (a, b) = (self.alias("e"), self.alias("e"));
                let (Some((fa, ca)), Some((fb, cb))) =
                    (self.edge_source(edges, &a, false), self.edge_source(edges, &b, false))
                else {
                    return NO_PAIRS.to_owned();
                };
                format!(
                    "SELECT {a}.s AS s, {a}.s AS o FROM {fa} WHERE {} UNION SELECT {b}.o, {b}.o FROM {fb} WHERE {}",
                    and_all(&ca),
                    and_all(&cb)
                )
            }
        }
    }
}

/// A one-pattern BGP.
fn bgp1(g: GraphSel, s: PatTerm, p: TermId, o: PatTerm) -> sq_core::ir::Logical {
    sq_core::ir::Logical::Bgp(vec![sq_core::ir::QuadPat { g, s, p: PatTerm::Const(p), o }])
}
