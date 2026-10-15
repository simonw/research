//! Property paths → a derived table of `(s, o)` pairs.
//!
//! * `p`, `^p`, `!(…)`: scans of `main.rdf_quad`;
//! * `a/b` (a join), `a|b` (`UNION ALL`): bag semantics, as in the spec;
//! * `p+`, `p*`: `WITH RECURSIVE … UNION` (set semantics), seeded from a constant endpoint
//!   when there is one (forward from the subject, else backward from the object);
//! * `p*`, `p?` add the zero-length pairs: a constant endpoint matches itself even when it is
//!   absent from the graph; with both endpoints free every node (subject or object) of the
//!   active graph matches itself.
//!
//! Inside EXISTS an endpoint substituted by the enclosing row is treated as a constant
//! (correlated reference), as the spec's substitution semantics require.
use sq_core::ir::{GraphSel, PathPlan, PatTerm};
use sq_core::{Result, TermId};

use crate::sql::{Block, Col, Item};
use crate::translate::{Env, GraphCond, QuadSrc, Translator, bind, unsupported};

/// The edges a path walks.
enum Edges {
    /// `main.rdf_quad` rows with `g = <sql>`.
    Table(String),
    /// A derived table of merged `(s, p, o)` triples.
    Merged(String),
    /// An empty graph (only the zero-length pairs of constants exist).
    Nothing,
}

/// A relation of `(s, o)` pairs with no rows.
const NO_PAIRS: &str = "SELECT NULL AS s, NULL AS o WHERE 0";

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
fn zero_length(p: &PathPlan) -> bool {
    match p {
        PathPlan::ZeroOrMore(_) | PathPlan::ZeroOrOne(_) => true,
        PathPlan::Inv(x) => zero_length(x),
        PathPlan::Alt(a, b) => zero_length(a) || zero_length(b),
        PathPlan::Seq(a, b) => zero_length(a) && zero_length(b),
        PathPlan::Pred(_) | PathPlan::Nps(_) | PathPlan::OneOrMore(_) => false,
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
        let edges = match self.quad_source(g, env)? {
            QuadSrc::Nothing if !zero_length(path) => return Ok(Block::empty()),
            QuadSrc::Nothing => Edges::Nothing,
            QuadSrc::Table(GraphCond::Eq(g)) => Edges::Table(g),
            QuadSrc::Table(GraphCond::Var(_)) => return Err(unsupported("property paths inside GRAPH ?g")),
            QuadSrc::Merged(sql) => Edges::Merged(sql),
        };
        let zero = zero_length(path);
        let start = self.endpoint(s, env, zero)?;
        let end = self.endpoint(o, env, zero)?;
        let rel = self.path_rel(path, &edges, start.as_deref(), end.as_deref())?;
        let alias = self.alias("d");
        let mut b = Block::unit();
        b.items.push(Item::Derived { sql: rel, alias: alias.clone() });
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

    /// SQL with result columns `(s, o)`: the pairs of `p`, restricted to the known endpoints.
    fn path_rel(&mut self, p: &PathPlan, edges: &Edges, start: Option<&str>, end: Option<&str>) -> Result<String> {
        Ok(match p {
            PathPlan::Pred(pred) => {
                let v = self.param(*pred);
                self.edge_scan(edges, Some(format!("p = {v}")), start, end)
            }
            PathPlan::Nps(list) => {
                let cond = (!list.is_empty()).then(|| {
                    let ids: Vec<String> = list.iter().map(|id| self.param(*id)).collect();
                    format!("p NOT IN ({})", ids.join(", "))
                });
                self.edge_scan(edges, cond, start, end)
            }
            PathPlan::Inv(x) => {
                let inner = self.path_rel(x, edges, end, start)?;
                let a = self.alias("d");
                format!("SELECT {a}.o AS s, {a}.s AS o FROM ({inner}) AS {a}")
            }
            PathPlan::Seq(x, y) => {
                let left = self.path_rel(x, edges, start, None)?;
                let right = self.path_rel(y, edges, None, end)?;
                let (a, b) = (self.alias("d"), self.alias("d"));
                format!("SELECT {a}.s AS s, {b}.o AS o FROM ({left}) AS {a}, ({right}) AS {b} WHERE {a}.o = {b}.s")
            }
            PathPlan::Alt(x, y) => {
                let left = self.path_rel(x, edges, start, end)?;
                let right = self.path_rel(y, edges, start, end)?;
                format!("SELECT s, o FROM ({left}) UNION ALL SELECT s, o FROM ({right})")
            }
            PathPlan::OneOrMore(x) => self.closure(x, edges, start, end, false)?,
            PathPlan::ZeroOrMore(x) => self.closure(x, edges, start, end, true)?,
            PathPlan::ZeroOrOne(x) => {
                let one = self.path_rel(x, edges, start, end)?;
                let zero = self.zero_pairs(edges, start, end);
                format!("SELECT s, o FROM ({one}) UNION SELECT s, o FROM ({zero})")
            }
        })
    }

    /// `SELECT s, o` over the edges with an optional predicate condition.
    fn edge_scan(&mut self, edges: &Edges, pred: Option<String>, start: Option<&str>, end: Option<&str>) -> String {
        let (from, mut conds) = match edges {
            Edges::Table(g) => ("main.rdf_quad".to_owned(), vec![format!("g = {g}")]),
            Edges::Merged(sql) => (format!("({sql})"), Vec::new()),
            Edges::Nothing => return NO_PAIRS.to_owned(),
        };
        conds.extend(pred);
        conds.extend(start.map(|v| format!("s = {v}")));
        conds.extend(end.map(|v| format!("o = {v}")));
        let mut sql = format!("SELECT s, o FROM {from}");
        if !conds.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conds.join(" AND "));
        }
        sql
    }

    /// One step of a closure as a FROM entry (`main.rdf_quad` directly for `p` and `^p`, so
    /// each recursion probes an index).
    fn step(&mut self, x: &PathPlan, edges: &Edges) -> Result<Step> {
        let k = self.alias("k");
        let direct = direct_pred(x).and_then(|(p, inverse)| match edges {
            Edges::Table(g) => Some((p, inverse, format!("main.rdf_quad AS {k}"), vec![format!("{k}.g = {g}")])),
            Edges::Merged(sql) => Some((p, inverse, format!("({sql}) AS {k}"), Vec::new())),
            Edges::Nothing => None,
        });
        if let Some((p, inverse, from, mut conds)) = direct {
            let pv = self.param(p);
            conds.push(format!("{k}.p = {pv}"));
            let (s, o) = if inverse { ("o", "s") } else { ("s", "o") };
            return Ok(Step { from, s: format!("{k}.{s}"), o: format!("{k}.{o}"), conds });
        }
        let rel = self.path_rel(x, edges, None, None)?;
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
                    format!("SELECT o FROM ({})", self.path_rel(x, edges, Some(c), None)?)
                };
                let rec = format!("SELECT {} FROM {r}, {} WHERE {}", step.o, step.from, and_conds(format!("{} = {r}.n", step.s)));
                let filter = end.map(|e| format!(" WHERE n = {e}")).unwrap_or_default();
                format!("WITH RECURSIVE {r}(n) AS ({seed} UNION {rec}) SELECT {c} AS s, n AS o FROM {r}{filter}")
            }
            (None, Some(e)) => {
                let seed = if reflexive {
                    format!("SELECT {e}")
                } else {
                    format!("SELECT s FROM ({})", self.path_rel(x, edges, None, Some(e))?)
                };
                let rec = format!("SELECT {} FROM {r}, {} WHERE {}", step.s, step.from, and_conds(format!("{} = {r}.n", step.o)));
                format!("WITH RECURSIVE {r}(n) AS ({seed} UNION {rec}) SELECT n AS s, {e} AS o FROM {r}")
            }
            (None, None) => {
                let first = self.path_rel(x, edges, None, None)?;
                let rec = format!(
                    "SELECT {r}.s, {} FROM {r}, {} WHERE {}",
                    step.o,
                    step.from,
                    and_conds(format!("{} = {r}.o", step.s))
                );
                let closure =
                    format!("WITH RECURSIVE {r}(s, o) AS (SELECT s, o FROM ({first}) UNION {rec}) SELECT s, o FROM {r}");
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
                let (from, wh) = match edges {
                    Edges::Table(g) => ("main.rdf_quad".to_owned(), format!(" WHERE g = {g}")),
                    Edges::Merged(sql) => (format!("({sql})"), String::new()),
                    Edges::Nothing => return NO_PAIRS.to_owned(),
                };
                format!("SELECT s, s AS o FROM {from}{wh} UNION SELECT o, o FROM {from}{wh}")
            }
        }
    }
}

/// A one-pattern BGP.
fn bgp1(g: GraphSel, s: PatTerm, p: TermId, o: PatTerm) -> sq_core::ir::Logical {
    sq_core::ir::Logical::Bgp(vec![sq_core::ir::QuadPat { g, s, p: PatTerm::Const(p), o }])
}
