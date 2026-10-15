//! Logical IR → one SQL statement (§9).
//!
//! Graph patterns become [`Block`]s (see `sql`); solution modifiers become a [`Stmt`] around a
//! block. Variable bindings follow SPARQL compatibility: a slot bound on both sides of a join
//! must be equal, and when either side may leave it unbound the condition is the nullable form
//! `(a IS NULL OR b IS NULL OR a = b)` with `COALESCE` for the merged value.
//!
//! EXISTS patterns are translated in the enclosing row's environment ([`Env::outer`]): every
//! slot bound there acts as a constant inside the pattern (substitution semantics, spec 18.6),
//! through correlated references, which SQLite allows from any depth of an EXISTS subquery.
//!
//! A BIND value is computed once per row: its expression stays in the block's SELECT list
//! until something reads the variable, and then the block becomes an opaque derived table
//! ([`Translator::settle`]) whose column is read instead. Every expression's text therefore
//! appears once in the statement, whatever the number of references to it.
use std::collections::BTreeMap;
use std::rc::Rc;

use sq_core::codec::encode_short_str;
use sq_core::ir::{
    AggFunc, AggSpec, DefaultGraph, Expr, GraphSel, Logical, LoweredQuery, NamedGraphs, PatTerm, QuadPat, QueryForm,
    VarOrigin,
};
use sq_core::{Error, Result, Slot, TermId};

use crate::SqlQuery;
use crate::expr::{Ctx, TermSql, View};
use crate::graph::{GClass, classify, expr_mentions, mentions};
use crate::helpers::{INT_TAG, aggregate_name};
use crate::limits;
use crate::sql::{Block, Col, Item, MAX_TABLES, PLACEHOLDER, Params, and_all, col_name, or_all, union_all};

/// `Error::Unsupported("baseline: …")`.
pub(crate) fn unsupported(what: impl std::fmt::Display) -> Error {
    Error::Unsupported(format!("baseline: {what}"))
}

/// The active graph of the patterns being translated.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum GraphCtx {
    /// The dataset's default graph.
    Default,
    /// `GRAPH <c>`.
    Const(TermId),
    /// `GRAPH ?g` pushed into the patterns: they bind the slot and range over named graphs.
    Var(Slot),
}

/// Translation environment of a pattern.
#[derive(Clone, Debug)]
pub(crate) struct Env {
    pub graph: GraphCtx,
    /// EXISTS substitution: the enclosing rows' bindings, visible everywhere in the pattern.
    pub outer: Rc<BTreeMap<Slot, Col>>,
}

impl Env {
    fn top() -> Env {
        Env { graph: GraphCtx::Default, outer: Rc::new(BTreeMap::new()) }
    }

    fn with_graph(&self, graph: GraphCtx) -> Env {
        Env { graph, outer: self.outer.clone() }
    }
}

/// Join condition and merged binding of a slot bound on two sides.
pub(crate) fn compat(a: &Col, b: &Col) -> (Option<String>, Col) {
    if a.sql == b.sql {
        return (None, if a.nullable { b.clone() } else { a.clone() });
    }
    let (x, y) = (&a.sql, &b.sql);
    match (a.nullable, b.nullable) {
        (false, false) => (Some(format!("{x} = {y}")), a.clone()),
        (false, true) => (Some(format!("({y} IS NULL OR {y} = {x})")), a.clone()),
        (true, false) => (Some(format!("({x} IS NULL OR {x} = {y})")), b.clone()),
        (true, true) => (Some(format!("({x} IS NULL OR {y} IS NULL OR {x} = {y})")), Col::coalesce(a, b, true)),
    }
}

/// Adds a binding to a block, joining it with an existing binding of the slot.
fn merge(b: &mut Block, s: Slot, col: Col) {
    match b.cols.get(&s) {
        None => {
            b.cols.insert(s, col);
        }
        Some(existing) => {
            let (cond, merged) = compat(existing, &col);
            b.conds.extend(cond);
            b.cols.insert(s, merged);
        }
    }
}

/// Binds a slot at a leaf (pattern position, VALUES column, BIND target, derived-table
/// column): joins with the block's existing binding and, the first time, with the EXISTS
/// substitution value of the slot.
pub(crate) fn bind(b: &mut Block, s: Slot, col: Col, env: &Env) {
    if !b.cols.contains_key(&s)
        && let Some(outer) = env.outer.get(&s)
    {
        let (cond, _) = compat(outer, &col);
        b.conds.extend(cond);
    }
    merge(b, s, col);
}

/// Natural (compatible-mapping) join of two blocks. Every LEFT JOIN's ON clause references only
/// its own block, so concatenating FROM lists computes `a ⋈ b` exactly.
pub(crate) fn join(mut a: Block, b: Block) -> Block {
    if a.empty || b.empty {
        return Block::empty();
    }
    a.items.extend(b.items);
    a.conds.extend(b.conds);
    for (s, c) in b.cols {
        merge(&mut a, s, c);
    }
    a
}

/// One ORDER BY key.
#[derive(Debug)]
pub(crate) struct OrderSql {
    pub sql: String,
    pub desc: bool,
    /// Slots the key reads (None: unknown, e.g. it contains EXISTS).
    pub slots: Option<Vec<Slot>>,
}

/// A block with solution modifiers (projection, DISTINCT, ORDER BY, LIMIT/OFFSET).
#[derive(Debug)]
pub(crate) struct Stmt {
    pub block: Block,
    pub proj: Option<Vec<Slot>>,
    pub distinct: bool,
    pub order: Vec<OrderSql>,
    /// (offset, limit).
    pub slice: Option<(u64, Option<u64>)>,
}

impl Stmt {
    fn of(block: Block) -> Stmt {
        Stmt { block, proj: None, distinct: false, order: Vec::new(), slice: None }
    }

    fn is_plain(&self) -> bool {
        self.proj.is_none() && !self.distinct && self.order.is_empty() && self.slice.is_none()
    }
}

/// Where the quads of a pattern come from.
pub(crate) enum QuadSrc {
    /// `main.rdf_quad` restricted by a graph condition.
    Table(GraphCond),
    /// The RDF merge of several graphs (see [`Translator::merge_conds`]).
    Merged(Merge),
    /// No triples at all (an empty default graph or no named graphs).
    Nothing,
}

/// The graphs an RDF merge reads.
#[derive(Clone, Debug)]
pub(crate) enum Merge {
    /// Every graph of the store, the default graph included (`union_default_graph`).
    All,
    /// The FROM graphs (placeholders).
    Graphs(Vec<String>),
}

/// The distinct graph ids of the store, by a loose index scan (one B-tree seek per graph).
const ALL_GRAPHS: &str = "WITH RECURSIVE gs(g) AS (SELECT min(g) FROM main.rdf_quad UNION ALL SELECT (SELECT \
                          min(g) FROM main.rdf_quad WHERE g > gs.g) FROM gs WHERE gs.g IS NOT NULL) SELECT g \
                          FROM gs WHERE g IS NOT NULL";

/// Graph restriction of a `main.rdf_quad` alias.
pub(crate) enum GraphCond {
    /// `g = <sql>`.
    Eq(String),
    /// `GRAPH ?g` pushed: `g` ranges over the dataset's named graphs and binds the slot.
    Var(Slot),
}

pub(crate) struct Translator<'q> {
    pub q: &'q LoweredQuery,
    pub params: Params,
    next_alias: u32,
}

/// Saturating u64 → i64 for LIMIT / OFFSET.
fn clamp(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

impl<'q> Translator<'q> {
    pub(crate) fn new(q: &'q LoweredQuery) -> Self {
        Translator { q, params: Params::default(), next_alias: 0 }
    }

    pub(crate) fn run(mut self) -> Result<SqlQuery> {
        let q = self.q;
        let env = Env::top();
        let (sql, columns) = match &q.form {
            QueryForm::Select(slots) => {
                let stmt = self.stmt(&q.root, &env)?;
                let sql = self.render_stmt(stmt, slots);
                (sql, slots.iter().map(|s| q.vars.name(*s).to_owned()).collect())
            }
            QueryForm::Ask => {
                let mut stmt = self.stmt(&q.root, &env)?;
                if stmt.slice.is_some() {
                    stmt = Stmt::of(self.derive_stmt(stmt, &env));
                }
                stmt.slice = Some((0, Some(1)));
                (self.render_stmt(stmt, &[]), Vec::new())
            }
            QueryForm::Construct(_) => return Err(unsupported("CONSTRUCT queries")),
            QueryForm::Describe(_) => return Err(unsupported("DESCRIBE queries")),
        };
        limits::check_size(sql.len())?;
        let (sql, params) = self.params.finish(&sql);
        limits::check_params(params.len())?;
        limits::check(&sql)?;
        Ok(SqlQuery { sql, params, columns })
    }

    /// A fresh alias (`t3`, `d4`, `r5`, …), unique in the statement.
    pub(crate) fn alias(&mut self, prefix: &str) -> String {
        self.next_alias += 1;
        format!("{prefix}{}", self.next_alias)
    }

    /// Placeholder of a term id.
    pub(crate) fn param(&mut self, id: TermId) -> String {
        self.params.get(id.raw())
    }

    // ------------------------------------------------------------------ statements

    /// A solution-modifier chain (Slice / Distinct / Reduced / Project / OrderBy) around a
    /// block, in SPARQL's order; anything out of that order is nested as a derived table.
    pub(crate) fn stmt(&mut self, node: &Logical, env: &Env) -> Result<Stmt> {
        Ok(match node {
            Logical::Slice { input, offset, limit } => {
                let mut s = self.stmt(input, env)?;
                if s.slice.is_some() {
                    s = Stmt::of(self.derive_stmt(s, env));
                }
                s.slice = Some((*offset, *limit));
                s
            }
            Logical::Distinct(input) => {
                let mut s = self.stmt(input, env)?;
                if s.slice.is_some() || s.distinct {
                    s = Stmt::of(self.derive_stmt(s, env));
                }
                s.distinct = true;
                s
            }
            // REDUCED permits but does not require eliminating duplicates.
            Logical::Reduced(input) => self.stmt(input, env)?,
            Logical::Project { input, slots } => {
                let mut s = self.stmt(input, env)?;
                if s.proj.is_some() || s.distinct || s.slice.is_some() {
                    s = Stmt::of(self.derive_stmt(s, env));
                }
                s.proj = Some(slots.clone());
                s
            }
            Logical::OrderBy { input, keys } => {
                let mut s = self.stmt(input, env)?;
                if !s.is_plain() {
                    s = Stmt::of(self.derive_stmt(s, env));
                }
                let q = self.q;
                s.block = self.settle(s.block, |x| keys.iter().any(|k| expr_mentions(&k.expr, x, &q.exists)), env);
                let mut order = Vec::with_capacity(keys.len());
                for k in keys {
                    let t = self.term_expr(&k.expr, &View::new(&s.block.cols, env))?;
                    let slots = (!k.expr.contains_exists()).then(|| {
                        let mut v = Vec::new();
                        k.expr.slots(&mut v);
                        v
                    });
                    order.push(OrderSql { sql: format!("sparql_sortkey({})", t.sql), desc: k.desc, slots });
                }
                s.order = order;
                s
            }
            _ => Stmt::of(self.block(node, env)?),
        })
    }

    /// Renders a statement whose result columns are `out` (`v<slot>`, NULL when unbound).
    pub(crate) fn render_stmt(&mut self, s: Stmt, out: &[Slot]) -> String {
        let order_list = |order: &[OrderSql]| {
            order
                .iter()
                .map(|k| if k.desc { format!("{} DESC", k.sql) } else { k.sql.clone() })
                .collect::<Vec<_>>()
                .join(", ")
        };
        // DISTINCT ordered by something outside the projection: keep each distinct row at its
        // first position in the ORDER BY sequence.
        let by_first_position = s.distinct
            && !out.is_empty()
            && s.order.iter().any(|k| k.slots.as_ref().is_none_or(|v| v.iter().any(|x| !out.contains(x))));
        let mut sql = if by_first_position {
            let cols = out.iter().map(|x| col_name(*x)).collect::<Vec<_>>().join(", ");
            format!(
                "SELECT {cols} FROM (SELECT {}, row_number() OVER (ORDER BY {}) AS rn{}{}) GROUP BY {cols} \
                 ORDER BY min(rn)",
                s.block.select_list(out),
                order_list(&s.order),
                s.block.from_sql(),
                s.block.where_sql()
            )
        } else {
            let mut sql = s.block.render(out, s.distinct);
            if !s.order.is_empty() {
                sql.push_str(" ORDER BY ");
                sql.push_str(&order_list(&s.order));
            }
            sql
        };
        if let Some((offset, limit)) = s.slice {
            let limit = self.params.get(limit.map_or(-1, clamp));
            sql.push_str(&format!(" LIMIT {limit}"));
            if offset > 0 {
                let offset = self.params.get(clamp(offset));
                sql.push_str(&format!(" OFFSET {offset}"));
            }
        }
        sql
    }

    /// A statement as a block: itself when it has no modifiers (an ORDER BY without a slice
    /// is meaningless in a derived table), else a derived table of its projection (opaque when
    /// it computes a projected value, see [`Block::computes`]).
    fn derive_stmt(&mut self, s: Stmt, env: &Env) -> Block {
        if s.proj.is_none() && !s.distinct && s.slice.is_none() {
            return s.block;
        }
        if s.block.empty {
            return Block::empty();
        }
        let out = s.proj.clone().unwrap_or_else(|| s.block.slots());
        let bound: Vec<(Slot, bool)> =
            out.iter().filter_map(|x| s.block.cols.get(x).map(|c| (*x, c.nullable))).collect();
        let opaque = s.slice.is_none() && s.block.computes(&out);
        let tables = if opaque { 1 } else { s.block.tables() };
        let mut sql = self.render_stmt(s, &out);
        if opaque {
            sql.push_str(" LIMIT -1");
        }
        self.derived(sql, bound, tables, env)
    }

    /// A derived-table block over `sql`, whose result columns `v<slot>` bind `slots`
    /// (`(slot, nullable)`); `tables` bounds the tables SQLite may flatten it into.
    pub(crate) fn derived(
        &mut self,
        sql: String,
        slots: impl IntoIterator<Item = (Slot, bool)>,
        tables: usize,
        env: &Env,
    ) -> Block {
        let alias = self.alias("d");
        let mut b = Block::unit();
        for (s, nullable) in slots {
            let col = format!("{alias}.{}", col_name(s));
            bind(&mut b, s, if nullable { Col::nullable(col) } else { Col::new(col) }, env);
        }
        b.items.insert(0, Item::Derived { sql, alias, tables });
        b
    }

    /// A block as a single derived table (all its bindings exposed). An `opaque` one carries
    /// `LIMIT -1`, which SQLite never flattens into a join: it counts as one table.
    fn derive_block(&mut self, b: Block, opaque: bool, env: &Env) -> Block {
        let slots = b.slots();
        let bound: Vec<(Slot, bool)> = b.cols.iter().map(|(s, c)| (*s, c.nullable)).collect();
        let mut sql = b.render(&slots, false);
        let tables = if opaque {
            sql.push_str(" LIMIT -1");
            1
        } else {
            b.tables()
        };
        self.derived(sql, bound, tables, env)
    }

    /// The block, or an opaque derived table of it when `reads` some slot whose binding is
    /// computed: the value is then computed once per row and read by column (see [`Col`]).
    /// Called before anything reads a block's bindings (an expression, a join condition).
    pub(crate) fn settle(&mut self, b: Block, reads: impl Fn(Slot) -> bool, env: &Env) -> Block {
        if !b.empty && b.cols.iter().any(|(s, c)| c.computed && reads(*s)) { self.derive_block(b, true, env) } else { b }
    }

    /// Keeps a block that is about to receive `more` tables within [`MAX_TABLES`] by turning
    /// what it has so far into one opaque derived table.
    fn cap_tables(&mut self, b: Block, more: usize, env: &Env) -> Block {
        if b.empty || b.tables() + more <= MAX_TABLES || b.tables() <= 1 {
            return b;
        }
        self.derive_block(b, true, env)
    }

    /// Natural join of two blocks within [`MAX_TABLES`].
    fn join_capped(&mut self, a: Block, b: Block, env: &Env) -> Block {
        // Slots bound on both sides are compared.
        let a = self.settle(a, |s| b.cols.contains_key(&s), env);
        let b = self.settle(b, |s| a.cols.contains_key(&s), env);
        let (a, b) = if a.tables() >= b.tables() {
            let a = self.cap_tables(a, b.tables(), env);
            let b = self.cap_tables(b, a.tables(), env);
            (a, b)
        } else {
            let b = self.cap_tables(b, a.tables(), env);
            let a = self.cap_tables(a, b.tables(), env);
            (a, b)
        };
        join(a, b)
    }

    // ------------------------------------------------------------------ graph patterns

    /// A graph pattern as a block, within [`limits::MAX_SQL_BYTES`].
    pub(crate) fn block(&mut self, node: &Logical, env: &Env) -> Result<Block> {
        let b = self.pattern(node, env)?;
        limits::check_size(b.text_len())?;
        Ok(b)
    }

    fn pattern(&mut self, node: &Logical, env: &Env) -> Result<Block> {
        match node {
            Logical::Unit => Ok(Block::unit()),
            Logical::Empty => Ok(Block::empty()),
            Logical::Bgp(pats) => self.bgp(pats, env),
            Logical::Path { g, s, path, o } => self.path(*g, *s, path, *o, env),
            Logical::Join(a, b) => {
                let a = self.block(a, env)?;
                let b = self.block(b, env)?;
                Ok(self.join_capped(a, b, env))
            }
            Logical::LeftJoin { left, right, filter } => self.left_join(left, right, filter.as_ref(), env),
            Logical::Minus { left, right } => self.minus(left, right, env),
            Logical::Union(..) => self.union(node, env),
            Logical::Filter { input, expr } => {
                let b = self.block(input, env)?;
                if b.empty {
                    return Ok(b);
                }
                let q = self.q;
                let mut b = self.settle(b, |s| expr_mentions(expr, s, &q.exists), env);
                let cond = self.bool_expr(expr, &View::new(&b.cols, env), Ctx::Truth)?;
                b.conds.push(cond);
                Ok(b)
            }
            Logical::Extend { input, slot, expr } => {
                let b = self.block(input, env)?;
                if b.empty {
                    return Ok(b);
                }
                let q = self.q;
                let b = self.settle(b, |s| s == *slot || expr_mentions(expr, s, &q.exists), env);
                let t = self.term_expr(expr, &View::new(&b.cols, env))?;
                Ok(self.extend(b, *slot, expr, t, env))
            }
            Logical::Values { slots, rows } => Ok(self.values(slots, rows, env)),
            Logical::Graph { name, input } => self.graph(*name, input, env),
            Logical::Group { input, keys, aggs } => self.group(input, keys, aggs, env),
            Logical::OrderBy { .. }
            | Logical::Project { .. }
            | Logical::Distinct(_)
            | Logical::Reduced(_)
            | Logical::Slice { .. } => {
                let s = self.stmt(node, env)?;
                Ok(self.derive_stmt(s, env))
            }
            Logical::Service { silent: true } => Ok(Block::unit()),
            Logical::Service { silent: false } => Err(unsupported("SERVICE")),
        }
    }

    /// BIND: binds `slot` to `expr`'s value `t`. A variable's binding, a constant or NULL is
    /// plain. A computed value stays in the block until something reads it (see
    /// [`Translator::settle`]); when it has to be compared with an EXISTS substitution of the
    /// slot right away, it is computed in an opaque derived table and compared by column.
    fn extend(&mut self, mut b: Block, slot: Slot, expr: &Expr, t: TermSql, env: &Env) -> Block {
        let plain = t.simple || matches!(expr, Expr::Slot(_));
        let col = Col { sql: t.sql, nullable: t.nullable, computed: !plain };
        if !col.computed || b.cols.contains_key(&slot) {
            // (BIND never rebinds an in-scope variable; any other rebinding is compared as is.)
            bind(&mut b, slot, col, env);
            return b;
        }
        b.cols.insert(slot, col);
        if env.outer.contains_key(&slot) { self.derive_block(b, true, env) } else { b }
    }

    /// Where the quads read through `sel` come from.
    pub(crate) fn quad_source(&mut self, sel: GraphSel, env: &Env) -> Result<QuadSrc> {
        let ctx = match sel {
            GraphSel::Default => GraphCtx::Default,
            GraphSel::Active => env.graph,
            GraphSel::Const(c) => GraphCtx::Const(c),
            GraphSel::Slot(_) => return Err(Error::internal("GraphSel::Slot in a logical pattern")),
        };
        Ok(match ctx {
            GraphCtx::Const(c) => QuadSrc::Table(GraphCond::Eq(self.param(c))),
            GraphCtx::Var(g) => match &self.q.dataset.named {
                NamedGraphs::Only(list) if list.is_empty() => QuadSrc::Nothing,
                _ => QuadSrc::Table(GraphCond::Var(g)),
            },
            GraphCtx::Default => match &self.q.dataset.default {
                DefaultGraph::StoreDefault => QuadSrc::Table(GraphCond::Eq("0".into())),
                DefaultGraph::UnionAll => QuadSrc::Merged(Merge::All),
                DefaultGraph::Graphs(list) => match list.len() {
                    0 => QuadSrc::Nothing,
                    1 => QuadSrc::Table(GraphCond::Eq(self.param(list[0]))),
                    _ => QuadSrc::Merged(Merge::Graphs(list.iter().map(|g| self.param(*g)).collect())),
                },
            },
        })
    }

    /// Conditions making the `main.rdf_quad` alias `alias` read an RDF merge: its graph is one
    /// of the merged graphs (an index prefix: `g IN (…)`) and, with `once`, it is the copy of
    /// its triple in the smallest of them, so that every triple of the merge is read exactly
    /// once (an index probe per row; g = 0, the default graph, sorts first).
    pub(crate) fn merge_conds(&mut self, m: &Merge, alias: &str, once: bool) -> Vec<String> {
        let mut conds = vec![match m {
            Merge::All => format!("{alias}.g IN ({ALL_GRAPHS})"),
            Merge::Graphs(list) => format!("{alias}.g IN ({})", list.join(", ")),
        }];
        if once {
            conds.push(match m {
                // Default-graph copies through the primary key, named-graph copies through the
                // (s, p, o, g) index of g <> 0.
                Merge::All => {
                    let default = self.no_copy(alias, |u| format!("{u}.g = 0"));
                    let named = self.no_copy(alias, |u| format!("{u}.g <> 0 AND {u}.g < {alias}.g"));
                    format!("({alias}.g = 0 OR {default} AND {named})")
                }
                Merge::Graphs(list) => {
                    self.no_copy(alias, |u| format!("{u}.g IN ({}) AND {u}.g < {alias}.g", list.join(", ")))
                }
            });
        }
        conds
    }

    /// `NOT EXISTS` a copy of `alias`'s triple in a graph that `graphs(u)` accepts.
    fn no_copy(&mut self, alias: &str, graphs: impl FnOnce(&str) -> String) -> String {
        let u = self.alias("u");
        format!(
            "NOT EXISTS (SELECT 1 FROM main.rdf_quad AS {u} WHERE {} AND {u}.s = {alias}.s AND {u}.p = {alias}.p \
             AND {u}.o = {alias}.o)",
            graphs(&u)
        )
    }

    /// Conditions restricting `alias.g` to the dataset's named graphs (GRAPH ?g).
    fn named_graph_conds(&mut self, alias: &str, b: &mut Block) {
        b.conds.push(format!("{alias}.g <> 0"));
        if let NamedGraphs::Only(list) = &self.q.dataset.named {
            let ids: Vec<String> = list.iter().map(|g| self.param(*g)).collect();
            b.conds.push(format!("{alias}.g IN ({})", ids.join(", ")));
        }
    }

    /// A basic graph pattern: one `main.rdf_quad` alias per triple pattern.
    fn bgp(&mut self, pats: &[QuadPat], env: &Env) -> Result<Block> {
        // A TEMP constant is absent from the store: the pattern cannot match.
        let absent = |t: &PatTerm| matches!(t, PatTerm::Const(c) if c.is_temp());
        if pats.iter().any(|p| absent(&p.s) || absent(&p.p) || absent(&p.o)) {
            return Ok(Block::empty());
        }
        let mut b = Block::unit();
        for p in pats {
            b = self.cap_tables(b, 1, env);
            let alias = self.alias("t");
            match self.quad_source(p.g, env)? {
                QuadSrc::Nothing => return Ok(Block::empty()),
                QuadSrc::Table(cond) => {
                    b.items.push(Item::Quad(alias.clone()));
                    match cond {
                        GraphCond::Eq(g) => b.conds.push(format!("{alias}.g = {g}")),
                        GraphCond::Var(slot) => {
                            self.named_graph_conds(&alias, &mut b);
                            bind(&mut b, slot, Col::new(format!("{alias}.g")), env);
                        }
                    }
                }
                QuadSrc::Merged(m) => {
                    b.items.push(Item::Quad(alias.clone()));
                    let conds = self.merge_conds(&m, &alias, true);
                    b.conds.extend(conds);
                }
            }
            for (pos, term) in [("s", p.s), ("p", p.p), ("o", p.o)] {
                let col = format!("{alias}.{pos}");
                match term {
                    PatTerm::Const(id) => {
                        let v = self.param(id);
                        b.conds.push(format!("{col} = {v}"));
                    }
                    PatTerm::Slot(x) => bind(&mut b, x, Col::new(col), env),
                }
            }
            // An RDF merge repeats its graph list for every pattern.
            limits::check_size(b.text_len())?;
        }
        Ok(b)
    }

    /// OPTIONAL: `LEFT JOIN <right> ON <right's conditions> AND <compatibility> AND <filter>`.
    fn left_join(
        &mut self,
        left: &Logical,
        right: &Logical,
        filter: Option<&sq_core::ir::Expr>,
        env: &Env,
    ) -> Result<Block> {
        let lb = self.block(left, env)?;
        if lb.empty {
            return Ok(lb);
        }
        let rb = self.block(right, env)?;
        if rb.empty {
            return Ok(lb);
        }
        let rb = if rb.is_single_item() {
            rb
        } else {
            let opaque = rb.computes(&rb.slots());
            self.derive_block(rb, opaque, env)
        };
        // The ON clause reads the shared slots and the filter's.
        let q = self.q;
        let reads = |s: Slot| rb.cols.contains_key(&s) || filter.is_some_and(|f| expr_mentions(f, s, &q.exists));
        let lb = self.settle(lb, reads, env);
        // Both sides within MAX_TABLES together (before any condition refers to them).
        let mut lb = self.cap_tables(lb, rb.tables(), env);
        let rb = if lb.tables() + rb.tables() > MAX_TABLES { self.derive_block(rb, true, env) } else { rb };
        let Block { items, conds, cols, .. } = rb;
        let item = items.into_iter().next().expect("single FROM item");
        let mut on = conds;
        // The filter sees the merged row.
        let mut merged = lb.cols.clone();
        let mut out = Vec::with_capacity(cols.len());
        for (s, rc) in cols {
            match lb.cols.get(&s) {
                Some(lc) => {
                    let (cond, m) = compat(lc, &rc);
                    on.extend(cond);
                    merged.insert(s, m);
                    if lc.nullable {
                        out.push((s, Col::coalesce(lc, &rc, true)));
                    }
                }
                None => {
                    merged.insert(s, rc.clone());
                    out.push((s, Col { nullable: true, ..rc }));
                }
            }
        }
        if let Some(f) = filter {
            on.push(self.bool_expr(f, &View::new(&merged, env), Ctx::Truth)?);
        }
        if lb.items.is_empty() {
            let alias = self.alias("d");
            lb.items.push(Item::Derived { sql: "SELECT 1".into(), alias, tables: 1 });
        }
        lb.items.push(Item::Left { item: Box::new(item), on: and_all(&on) });
        lb.cols.extend(out);
        Ok(lb)
    }

    /// MINUS: `NOT EXISTS (<right> AND <compatible> AND <shared domain non-empty>)`.
    fn minus(&mut self, left: &Logical, right: &Logical, env: &Env) -> Result<Block> {
        let lb = self.block(left, env)?;
        if lb.empty {
            return Ok(lb);
        }
        let rb = self.block(right, env)?;
        if rb.empty {
            return Ok(lb);
        }
        // The shared slots are compared.
        let mut lb = self.settle(lb, |s| rb.cols.contains_key(&s), env);
        let rb = self.settle(rb, |s| lb.cols.contains_key(&s), env);
        let mut conds = rb.conds.clone();
        // Per shared variable: its compatibility condition and its shared-domain term (None
        // when it is certainly bound on both sides, which makes the domain non-empty).
        let mut shared: Vec<(Option<String>, Option<String>)> = Vec::new();
        for (s, rc) in &rb.cols {
            let Some(lc) = lb.cols.get(s) else { continue };
            if env.graph == GraphCtx::Var(*s) {
                // The pushed GRAPH ?g slot: both sides read the same graph; it is not a
                // variable of either MINUS operand.
                conds.push(format!("{} = {}", lc.sql, rc.sql));
                continue;
            }
            let seeded = env.outer.get(s);
            if seeded.is_some_and(|o| !o.nullable) {
                // Substituted by EXISTS: a constant on both sides, outside the domain test.
                continue;
            }
            let (cond, _) = compat(lc, rc);
            let eq = format!("{} = {}", lc.sql, rc.sql);
            let domain = match seeded {
                Some(o) => Some(format!("({} IS NULL AND {eq})", o.sql)),
                None if !lc.nullable && !rc.nullable => None,
                None => Some(eq),
            };
            shared.push((cond, domain));
        }
        let certain_shared = shared.iter().any(|(_, d)| d.is_none());
        let domain: Vec<String> = shared.iter().filter_map(|(_, d)| d.clone()).collect();
        if !certain_shared && domain.is_empty() {
            // dom(left) and dom(right) can never intersect: MINUS removes nothing.
            return Ok(lb);
        }
        if certain_shared || domain.len() > 1 {
            conds.extend(shared.into_iter().filter_map(|(c, _)| c));
        }
        // A single domain term is an equality, which implies its slot's compatibility.
        if !certain_shared {
            conds.push(or_all(&domain));
        }
        let wh = if conds.is_empty() { String::new() } else { format!(" WHERE {}", and_all(&conds)) };
        lb.conds.push(format!("NOT EXISTS (SELECT 1{}{wh})", rb.from_sql()));
        Ok(lb)
    }

    /// UNION: `UNION ALL` of the branches, NULL-padded to the union of their slots.
    fn union(&mut self, node: &Logical, env: &Env) -> Result<Block> {
        fn flatten<'a>(n: &'a Logical, out: &mut Vec<&'a Logical>) {
            match n {
                Logical::Union(a, b) => {
                    flatten(a, out);
                    flatten(b, out);
                }
                other => out.push(other),
            }
        }
        let mut branches = Vec::new();
        flatten(node, &mut branches);
        let mut blocks = Vec::with_capacity(branches.len());
        for br in branches {
            let b = self.block(br, env)?;
            if !b.empty {
                blocks.push(b);
            }
        }
        if blocks.len() <= 1 {
            return Ok(blocks.pop().unwrap_or_else(Block::empty));
        }
        let mut slots: BTreeMap<Slot, bool> = BTreeMap::new();
        for b in &blocks {
            for (s, c) in &b.cols {
                *slots.entry(*s).or_insert(false) |= c.nullable;
            }
        }
        for (s, nullable) in slots.iter_mut() {
            *nullable |= blocks.iter().any(|b| !b.cols.contains_key(s));
        }
        let out: Vec<Slot> = slots.keys().copied().collect();
        let mut sql = union_all(blocks.iter().map(|b| b.render(&out, false)).collect());
        let tables = if blocks.iter().any(|b| b.computes(&out)) {
            // Opaque: see `Block::computes`.
            sql.push_str(" LIMIT -1");
            1
        } else {
            // SQLite may flatten a UNION ALL into its parent, one copy of the parent per branch.
            blocks.iter().map(Block::tables).max().unwrap_or(1)
        };
        Ok(self.derived(sql, slots, tables, env))
    }

    /// VALUES: constants for a single row, else a `VALUES` derived table (UNDEF = NULL).
    fn values(&mut self, slots: &[Slot], rows: &[Vec<TermId>], env: &Env) -> Block {
        let mut b = Block::unit();
        match rows {
            [] => return Block::empty(),
            [row] => {
                for (s, id) in slots.iter().zip(row) {
                    if id.is_bound() {
                        let v = self.param(*id);
                        bind(&mut b, *s, Col::new(v), env);
                    }
                }
                return b;
            }
            _ => {}
        }
        let mut sql = String::from("VALUES ");
        for (i, row) in rows.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            let cells: Vec<String> = if slots.is_empty() {
                vec!["NULL".into()]
            } else {
                row.iter().map(|id| if id.is_bound() { self.param(*id) } else { "NULL".into() }).collect()
            };
            sql.push_str(&format!("({})", cells.join(", ")));
        }
        let alias = self.alias("d");
        for (i, s) in slots.iter().enumerate() {
            let bound = rows.iter().filter(|r| r[i].is_bound()).count();
            if bound > 0 {
                let col = format!("{alias}.column{}", i + 1);
                bind(&mut b, *s, if bound < rows.len() { Col::nullable(col) } else { Col::new(col) }, env);
            }
        }
        b.items.insert(0, Item::Derived { sql, alias, tables: 1 });
        b
    }

    /// `GRAPH <c>` / `GRAPH ?g`.
    fn graph(&mut self, name: PatTerm, input: &Logical, env: &Env) -> Result<Block> {
        let q = self.q;
        match name {
            PatTerm::Const(c) => {
                // Dataset membership: FROM NAMED lists it statically (absent IRIs are empty
                // named graphs); otherwise c must be a named graph of the store.
                let check = match &q.dataset.named {
                    NamedGraphs::Only(list) if !list.contains(&c) => return Ok(Block::empty()),
                    NamedGraphs::Only(_) => false,
                    NamedGraphs::All if c.is_temp() => return Ok(Block::empty()),
                    NamedGraphs::All => true,
                };
                let mut b = self.block(input, &env.with_graph(GraphCtx::Const(c)))?;
                // A GForm body reads a quad of graph c in every solution, which proves that c
                // exists; any other body (including a zero-length path, whose pairs of a
                // constant endpoint read no quad) needs the explicit check.
                if check && !b.empty && classify(input, &q.exists) != GClass::GForm {
                    let g = self.param(c);
                    b.conds.push(format!(
                        "(EXISTS (SELECT 1 FROM main.rdf_graph WHERE g = {g}) OR \
                         EXISTS (SELECT 1 FROM main.rdf_quad WHERE g = {g}))"
                    ));
                }
                Ok(b)
            }
            // No named graphs (FROM without FROM NAMED): `GRAPH ?g` has no solutions at all.
            PatTerm::Slot(_) if matches!(&q.dataset.named, NamedGraphs::Only(list) if list.is_empty()) => {
                Ok(Block::empty())
            }
            PatTerm::Slot(g) => match classify(input, &q.exists) {
                GClass::GForm => {
                    if mentions(input, g, &q.exists) {
                        return Err(unsupported("GRAPH ?g whose pattern also uses ?g"));
                    }
                    self.block(input, &env.with_graph(GraphCtx::Var(g)))
                }
                GClass::Indep => {
                    let b = self.block(input, env)?;
                    let names = self.named_graphs(g, env);
                    Ok(self.join_capped(b, names, env))
                }
                GClass::Bad => {
                    Err(unsupported("GRAPH ?g over a pattern that does not read the graph in every solution"))
                }
            },
        }
    }

    /// The dataset's named graphs, binding `g`.
    fn named_graphs(&mut self, g: Slot, env: &Env) -> Block {
        match &self.q.dataset.named {
            NamedGraphs::All => {
                let sql = format!(
                    "SELECT g AS {} FROM main.rdf_graph UNION SELECT g FROM main.rdf_quad WHERE g <> 0",
                    col_name(g)
                );
                self.derived(sql, [(g, false)], 1, env)
            }
            NamedGraphs::Only(list) => {
                let rows: Vec<Vec<TermId>> = list.iter().map(|id| vec![*id]).collect();
                self.values(&[g], &rows, env)
            }
        }
    }

    // ------------------------------------------------------------------ aggregates

    /// GROUP BY: a derived table of the group keys and aggregate values.
    fn group(&mut self, input: &Logical, keys: &[Slot], aggs: &[(Slot, AggSpec)], env: &Env) -> Result<Block> {
        let b = self.block(input, env)?;
        if b.empty && !keys.is_empty() {
            return Ok(Block::empty());
        }
        let q = self.q;
        let reads = |s: Slot| {
            keys.contains(&s) || aggs.iter().any(|(_, a)| a.arg.as_ref().is_some_and(|e| expr_mentions(e, s, &q.exists)))
        };
        let mut b = self.settle(b, reads, env);
        if aggs.iter().any(|(_, a)| a.func == AggFunc::CountAll && a.distinct) {
            b = self.distinct_solutions(b, keys, aggs, env)?;
        }
        let view = View::new(&b.cols, env);
        let mut select = Vec::new();
        let mut group_by = Vec::new();
        let mut out = Vec::new();
        for k in keys {
            // A key that is never bound is constant (unbound) and needs no grouping.
            if let Some(c) = view.read(*k) {
                select.push(format!("{} AS {}", c.sql, col_name(*k)));
                group_by.push(c.sql);
                out.push((*k, c.nullable));
            }
        }
        for (s, spec) in aggs {
            let (sql, nullable) = self.aggregate(spec, &view)?;
            select.push(format!("{sql} AS {}", col_name(*s)));
            out.push((*s, nullable));
        }
        if select.is_empty() {
            select.push(PLACEHOLDER.to_owned());
        }
        if !keys.is_empty() && group_by.is_empty() {
            // Keys that are never bound form one group, but an empty input still has none
            // (without GROUP BY, SQL would return one row).
            group_by.push("NULL".to_owned());
        }
        let tables = b.tables();
        let mut sql = format!("SELECT {}{}{}", select.join(", "), b.from_sql(), b.where_sql());
        if !group_by.is_empty() {
            sql.push_str(" GROUP BY ");
            sql.push_str(&group_by.join(", "));
        }
        Ok(self.derived(sql, out, tables, env))
    }

    /// One aggregate: `(sql, nullable)`. COUNT is computed in SQL as an inline integer id;
    /// the others call the aggregate helpers.
    fn aggregate(&mut self, spec: &AggSpec, view: &View<'_>) -> Result<(String, bool)> {
        let distinct = if spec.distinct { "DISTINCT " } else { "" };
        let arg = |t: &mut Self| -> Result<String> {
            let e = spec.arg.as_ref().ok_or_else(|| Error::internal("aggregate without an argument"))?;
            Ok(t.term_expr(e, view)?.sql)
        };
        Ok(match &spec.func {
            // COUNT(DISTINCT *) runs over distinct solutions (see `distinct_solutions`).
            AggFunc::CountAll => (format!("(COUNT(*) << 4) | {INT_TAG}"), false),
            AggFunc::Count => (format!("(COUNT({distinct}{}) << 4) | {INT_TAG}", arg(self)?), false),
            // An unknown aggregate is an evaluation error: always unbound.
            AggFunc::Custom(_) => ("NULL".to_owned(), true),
            AggFunc::GroupConcat { separator } if separator != " " => {
                if spec.distinct {
                    return Err(unsupported("GROUP_CONCAT(DISTINCT …; SEPARATOR …)"));
                }
                let sep = encode_short_str(separator)
                    .ok_or_else(|| unsupported("GROUP_CONCAT separators longer than 7 bytes"))?;
                let e = arg(self)?;
                (format!("sparql_group_concat({e}, {})", self.param(sep)), true)
            }
            f => {
                let name = aggregate_name(f).ok_or_else(|| Error::internal("aggregate without a helper"))?;
                (format!("{name}({distinct}{})", arg(self)?), true)
            }
        })
    }

    /// COUNT(DISTINCT *): the group input reduced to distinct solutions over its user-visible
    /// slots. Only valid when every other aggregate ignores duplicates.
    fn distinct_solutions(&mut self, b: Block, keys: &[Slot], aggs: &[(Slot, AggSpec)], env: &Env) -> Result<Block> {
        let duplicate_insensitive = |a: &AggSpec| {
            a.distinct || matches!(a.func, AggFunc::Min | AggFunc::Max | AggFunc::Sample | AggFunc::Custom(_))
        };
        if !aggs.iter().all(|(_, a)| duplicate_insensitive(a)) {
            return Err(unsupported("COUNT(DISTINCT *) next to duplicate-sensitive aggregates"));
        }
        let vars = &self.q.vars.vars;
        let visible: Vec<Slot> = b
            .cols
            .keys()
            .copied()
            .filter(|s| matches!(vars[*s as usize].1, VarOrigin::User | VarOrigin::Renamed))
            .collect();
        let mut read = keys.to_vec();
        for (_, a) in aggs {
            if let Some(e) = &a.arg {
                if e.contains_exists() {
                    return Err(unsupported("COUNT(DISTINCT *) next to an aggregate over EXISTS"));
                }
                e.slots(&mut read);
            }
        }
        if read.iter().any(|s| b.cols.contains_key(s) && !visible.contains(s)) {
            return Err(unsupported("COUNT(DISTINCT *) with keys or arguments over hidden variables"));
        }
        let bound: Vec<(Slot, bool)> = visible.iter().map(|s| (*s, b.cols[s].nullable)).collect();
        let sql = b.render(&visible, true);
        Ok(self.derived(sql, bound, b.tables(), env))
    }
}
