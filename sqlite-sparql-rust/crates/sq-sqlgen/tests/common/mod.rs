//! Equivalence harness: the same data in a real SQLite store (sq-sqlite) and in an
//! `oxrdf::Dataset`; a query is lowered (sq-plan), translated (sq-sqlgen) and executed by
//! SQLite with TEST implementations of the helper functions (sq-expr semantics over a snapshot
//! of the store's dictionary), then compared with spareval 0.2.7 on the dataset.
#![allow(dead_code)]

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex, MutexGuard};

use oxrdf::{Dataset, GraphName, NamedNode, Quad};
use oxrdfio::{RdfFormat, RdfParser};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rusqlite::functions::{Aggregate, Context, FunctionFlags};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, params_from_iter};
use sq_core::codec::{self, RDF_LANG_STRING_IRI};
use sq_core::eval_api::{ExistsEval, TempDict, TermResolver};
use sq_core::ir::{AggFunc, AggSpec, ExistsId, LoweredQuery};
use sq_core::plan::QueryOptions;
use sq_core::{Error, Literal, Tag, Term, TermId};
use sq_expr::{Accumulator, CompiledExpr, EvalCtx, Val, ValueCache};
use sq_sqlgen::SqlQuery;
use sq_sqlite::{BulkMode, LoadOptions, LoadSource, RawDb, SqliteStore, StorageConfig, StorageShared};

/// One input file.
#[derive(Clone, Debug)]
pub struct DataFile {
    pub data: Vec<u8>,
    pub format: RdfFormat,
    pub base: Option<String>,
    /// Target named graph (None = default graph).
    pub graph: Option<String>,
}

impl DataFile {
    pub fn turtle(text: &str) -> DataFile {
        DataFile { data: text.as_bytes().to_vec(), format: RdfFormat::Turtle, base: None, graph: None }
    }

    pub fn trig(text: &str) -> DataFile {
        DataFile { data: text.as_bytes().to_vec(), format: RdfFormat::TriG, base: None, graph: None }
    }
}

/// A query's answer, decoded.
#[derive(Clone, Debug)]
pub enum Outcome {
    Solutions { vars: Vec<String>, rows: Vec<Vec<Option<oxrdf::Term>>> },
    Boolean(bool),
}

/// Store dictionary snapshot (the test helpers must be `Send`, the store is not).
#[derive(Default)]
pub struct Dict {
    by_id: HashMap<i64, Arc<Term>>,
    by_term: HashMap<Term, TermId>,
}

impl Dict {
    fn load(conn: &Connection) -> Dict {
        let mut stmt = conn.prepare("SELECT id, tag, lex, dt, lang FROM main.rdf_term ORDER BY id").unwrap();
        let rows: Vec<(i64, i64, String, Option<i64>, Option<String>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let iris: HashMap<i64, String> =
            rows.iter().filter(|r| r.1 == Tag::Iri as i64).map(|r| ((r.0 << 4) | 1, r.2.clone())).collect();
        let mut d = Dict::default();
        for (row, tag, lex, dt, lang) in rows {
            let term = match tag {
                1 => Term::Iri(lex.as_str().into()),
                3 => Term::Literal(Literal::simple(&lex)),
                4 => Term::Literal(Literal::lang(&lex, lang.as_deref().unwrap_or(""))),
                5 => Term::Literal(Literal::typed(&lex, &iris[&dt.expect("TYPED row has a datatype")])),
                _ => panic!("unexpected dictionary tag {tag}"),
            };
            let id = TermId((row << 4) | tag);
            d.by_term.insert(term.clone(), id);
            d.by_id.insert(id.raw(), Arc::new(term));
        }
        d
    }
}

/// The resolver the test helpers evaluate with: dictionary snapshot + the query's TempDict.
struct Resolver<'a> {
    dict: &'a Dict,
    temps: &'a mut TempDict,
}

impl TermResolver for Resolver<'_> {
    fn term(&mut self, id: TermId) -> sq_core::Result<Arc<Term>> {
        if id.is_inline() {
            return codec::decode_inline(id).map(Arc::new).ok_or_else(|| Error::internal("bad inline id"));
        }
        match id.tag() {
            Tag::BNode => Ok(Arc::new(Term::BNode(codec::bnode_label(id).into()))),
            Tag::Temp => self.temps.get(id).cloned().ok_or_else(|| Error::internal("unknown TEMP id")),
            Tag::Iri | Tag::Str | Tag::Lang | Tag::Typed => {
                self.dict.by_id.get(&id.raw()).cloned().ok_or_else(|| Error::internal(format!("unknown id {}", id.0)))
            }
            _ => Err(Error::internal(format!("cannot decode id {}", id.0))),
        }
    }

    fn intern(&mut self, t: &Term, canonical: bool) -> sq_core::Result<TermId> {
        if let Term::Literal(l) = t
            && let Some(id) = codec::encode_inline_literal(&l.lex, &l.datatype, l.lang.as_deref())
        {
            return Ok(id);
        }
        if let Term::BNode(label) = t
            && let Some(id) = codec::parse_bnode_label(label)
        {
            return Ok(id);
        }
        if canonical && let Some(id) = self.dict.by_term.get(t) {
            return Ok(*id);
        }
        Ok(self.temps.intern(t.clone()))
    }

    fn fresh_bnode(&mut self) -> TermId {
        self.temps.fresh_bnode()
    }
}

struct NoExists;

impl ExistsEval for NoExists {
    fn exists(&mut self, _: ExistsId, _: &[TermId]) -> sq_core::Result<bool> {
        Err(Error::internal("helper templates never contain EXISTS"))
    }
}

/// Per-execution state of the test helpers (what sq-api keeps per baseline execution).
pub struct HelperState {
    dict: Arc<Dict>,
    temps: TempDict,
    values: ValueCache,
    rng: StdRng,
    now: oxsdatatypes::DateTime,
    base: Option<String>,
    compiled: HashMap<(String, usize), CompiledExpr>,
}

/// Shared by the registered closures (rusqlite requires `Send`; the state is `Send`).
type Shared = Arc<Mutex<HelperState>>;

fn lock(s: &Shared) -> MutexGuard<'_, HelperState> {
    s.lock().unwrap_or_else(|e| e.into_inner())
}

fn user_err(e: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::UserFunctionError(e.to_string().into())
}

impl HelperState {
    /// Runs `f` with an evaluation context over this state.
    fn with_cx<R>(&mut self, f: impl FnOnce(&mut EvalCtx<'_>) -> R) -> R {
        let HelperState { dict, temps, values, rng, now, base, .. } = self;
        let mut resolver = Resolver { dict, temps };
        let mut exists = NoExists;
        let mut cx = EvalCtx {
            resolver: &mut resolver,
            exists: &mut exists,
            values,
            now: *now,
            base_iri: base.as_deref(),
            rng,
        };
        f(&mut cx)
    }

    fn decode(&mut self, id: TermId) -> oxrdf::Term {
        let mut r = Resolver { dict: &self.dict, temps: &mut self.temps };
        r.term(id).unwrap().to_oxrdf()
    }
}

/// A term argument: INTEGER id or NULL.
fn id_arg(ctx: &Context<'_>, i: usize) -> rusqlite::Result<Option<TermId>> {
    match ctx.get_raw(i) {
        ValueRef::Null => Ok(None),
        ValueRef::Integer(v) => Ok(Some(TermId(v))),
        other => Err(user_err(format!("argument {i} is not a term id: {other:?}"))),
    }
}

fn sql_bool(b: Option<bool>) -> Option<i64> {
    b.map(i64::from)
}

/// Evaluates `sparql_eval(op, …)` through `sq_sqlgen::eval_template`.
fn eval_op(st: &mut HelperState, op: &str, args: &[TermId], as_condition: bool) -> rusqlite::Result<Option<i64>> {
    let key = (op.to_owned(), args.len());
    if !st.compiled.contains_key(&key) {
        let template = sq_sqlgen::eval_template(op, args.len()).ok_or_else(|| user_err(format!("unknown op {op}")))?;
        let compiled = sq_expr::compile(&template, &st.temps).map_err(user_err)?;
        st.compiled.insert(key.clone(), compiled);
    }
    // Take the compiled expression out while evaluating (the context borrows the state).
    let compiled = st.compiled.remove(&key).expect("just inserted");
    let out = st.with_cx(|cx| {
        if as_condition {
            compiled.ebv(args, cx).map(sql_bool)
        } else {
            compiled.eval(args, cx, true).map(|r| r.map(TermId::raw))
        }
    });
    st.compiled.insert(key, compiled);
    out.map_err(user_err)
}

/// Aggregate helper: an sq-expr accumulator per group.
struct AggHelper {
    state: Shared,
    func: AggFunc,
}

type Acc = AssertUnwindSafe<Box<dyn Accumulator>>;

impl Aggregate<Acc, Option<i64>> for AggHelper {
    fn init(&self, ctx: &mut Context<'_>) -> rusqlite::Result<Acc> {
        let mut func = self.func.clone();
        if let AggFunc::GroupConcat { separator } = &mut func
            && ctx.len() == 2
        {
            let sep = id_arg(ctx, 1)?.ok_or_else(|| user_err("NULL separator"))?;
            let term = lock(&self.state).with_cx(|cx| cx.resolver.term(sep)).map_err(user_err)?;
            let Term::Literal(l) = &*term else { return Err(user_err("separator is not a literal")) };
            *separator = l.lex.to_string();
        }
        Ok(AssertUnwindSafe(sq_expr::accumulator(&AggSpec { func, distinct: false, arg: None })))
    }

    fn step(&self, ctx: &mut Context<'_>, acc: &mut Acc) -> rusqlite::Result<()> {
        let v = id_arg(ctx, 0)?;
        lock(&self.state).with_cx(|cx| acc.add(v, cx)).map_err(user_err)
    }

    fn finalize(&self, ctx: &mut Context<'_>, acc: Option<Acc>) -> rusqlite::Result<Option<i64>> {
        let mut acc = match acc {
            Some(a) => a,
            None => self.init(ctx)?,
        };
        let out = lock(&self.state).with_cx(|cx| acc.finish(cx)).map_err(user_err)?;
        Ok(out.map(TermId::raw))
    }
}

/// Registers the test helpers (sq-sqlgen's `HELPER_FUNCTIONS`) over `state`.
fn register_helpers(conn: &Connection, state: &Shared) {
    let flags = FunctionFlags::SQLITE_UTF8;
    let s = state.clone();
    conn.create_scalar_function("sparql_num", 1, flags, move |ctx| {
        let Some(id) = id_arg(ctx, 0)? else { return Ok(SqlValue::Null) };
        let v = lock(&s).with_cx(|cx| sq_expr::value_of(id, cx)).map_err(user_err)?;
        Ok(match v.as_num() {
            Some(sq_expr::Num::Integer(i)) => SqlValue::Integer(i),
            Some(n) if n.is_nan() => SqlValue::Null,
            Some(n) => SqlValue::Real(n.to_f64()),
            None => SqlValue::Null,
        })
    })
    .unwrap();
    let s = state.clone();
    conn.create_scalar_function("sparql_str", 1, flags, move |ctx| {
        let Some(id) = id_arg(ctx, 0)? else { return Ok(None) };
        let v = lock(&s).with_cx(|cx| sq_expr::value_of(id, cx)).map_err(user_err)?;
        Ok(match v {
            sq_expr::Value::Str(s) => Some(s.to_string()),
            sq_expr::Value::LangStr { lex, .. } => Some(lex.to_string()),
            _ => None,
        })
    })
    .unwrap();
    let s = state.clone();
    conn.create_scalar_function("sparql_ebv", 1, flags, move |ctx| {
        let Some(id) = id_arg(ctx, 0)? else { return Ok(None) };
        lock(&s).with_cx(|cx| sq_expr::ebv(&Val::Id(id), cx)).map(sql_bool).map_err(user_err)
    })
    .unwrap();
    let s = state.clone();
    conn.create_scalar_function("sparql_sortkey", 1, flags, move |ctx| {
        let Some(id) = id_arg(ctx, 0)? else { return Ok(None) };
        lock(&s).with_cx(|cx| sq_expr::sort_key(id, cx)).map(|k| Some(k.into_bytes())).map_err(user_err)
    })
    .unwrap();
    let s = state.clone();
    conn.create_scalar_function("sparql_regex", -1, flags, move |ctx| {
        let args = (0..ctx.len()).map(|i| id_arg(ctx, i).map(Option::unwrap_or_default)).collect::<Result<Vec<_>, _>>()?;
        eval_op(&mut lock(&s), "REGEX", &args, true)
    })
    .unwrap();
    let s = state.clone();
    conn.create_scalar_function("sparql_eval", -1, flags, move |ctx| {
        let op: String = ctx.get(0)?;
        let args = (1..ctx.len()).map(|i| id_arg(ctx, i).map(Option::unwrap_or_default)).collect::<Result<Vec<_>, _>>()?;
        eval_op(&mut lock(&s), &op, &args, false)
    })
    .unwrap();
    let aggregates = [
        ("sparql_sum", AggFunc::Sum),
        ("sparql_avg", AggFunc::Avg),
        ("sparql_min", AggFunc::Min),
        ("sparql_max", AggFunc::Max),
        ("sparql_sample", AggFunc::Sample),
        ("sparql_group_concat", AggFunc::GroupConcat { separator: " ".into() }),
    ];
    for (name, func) in aggregates {
        conn.create_aggregate_function(name, -1, flags, AggHelper { state: state.clone(), func }).unwrap();
    }
}

/// A store and its reference copy. Field order matters: the store (pooled statements) drops
/// before the connection.
pub struct Engine {
    pub store: SqliteStore,
    pub conn: Connection,
    pub dataset: Dataset,
    dict: Arc<Dict>,
}

/// What the baseline produced for one query.
pub enum Baseline {
    Unsupported(String),
    Ran { sql: SqlQuery, outcome: Outcome },
}

impl Engine {
    pub fn new(files: &[DataFile]) -> Result<Engine, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        // SAFETY: `conn` outlives `store`: `Engine` declares (hence drops) the store first.
        let db = unsafe { RawDb::from_ptr(conn.handle()) };
        let shared = StorageShared::new(db, StorageConfig::default());
        shared.arm_pool();
        let store = SqliteStore::new(shared);
        store.ensure_schema().map_err(|e| e.to_string())?;
        let mut dataset = Dataset::new();
        for f in files {
            let opts = LoadOptions { format: f.format, base_iri: f.base.clone(), graph: f.graph.clone(), bulk: BulkMode::Never };
            store.bulk_load(LoadSource::Bytes(&f.data), &opts).map_err(|e| format!("sqlite load: {e}"))?;
            let mut parser = RdfParser::from_format(f.format).rename_blank_nodes();
            if let Some(base) = &f.base {
                parser = parser.with_base_iri(base).map_err(|e| e.to_string())?;
            }
            let target = match &f.graph {
                Some(g) => GraphName::NamedNode(NamedNode::new(g).map_err(|e| e.to_string())?),
                None => GraphName::DefaultGraph,
            };
            for q in parser.for_slice(&f.data) {
                let q = q.map_err(|e| format!("reference load: {e}"))?;
                let graph = if q.graph_name.is_default_graph() { target.clone() } else { q.graph_name };
                dataset.insert(&Quad::new(q.subject, q.predicate, q.object, graph));
            }
        }
        store.shared().validate_epoch().map_err(|e| e.to_string())?;
        let dict = Arc::new(Dict::load(&conn));
        Ok(Engine { store, conn, dataset, dict })
    }

    /// Lowers a query against the store.
    pub fn lower(&self, query: &spargebra::Query) -> Result<LoweredQuery, String> {
        sq_plan::lower_query(query, &self.store, &QueryOptions::default()).map_err(|e| e.to_string())
    }

    /// Lowers, translates and runs a query on SQLite.
    pub fn baseline(&self, query: &spargebra::Query) -> Result<Baseline, String> {
        let lowered = self.lower(query)?;
        let sql = match sq_sqlgen::translate(&lowered) {
            Ok(sql) => sql,
            Err(Error::Unsupported(msg)) => return Ok(Baseline::Unsupported(msg)),
            Err(e) => return Err(format!("translate: {e}")),
        };
        let outcome = self.execute(&lowered, &sql)?;
        Ok(Baseline::Ran { sql, outcome })
    }

    /// Executes translated SQL with the test helpers registered for this query.
    pub fn execute(&self, lowered: &LoweredQuery, sql: &SqlQuery) -> Result<Outcome, String> {
        let state: Shared = Arc::new(Mutex::new(HelperState {
            dict: self.dict.clone(),
            temps: lowered.temps.clone(),
            values: ValueCache::new(),
            rng: StdRng::seed_from_u64(1),
            now: "2026-10-15T12:00:00Z".parse().expect("valid dateTime"),
            base: lowered.base_iri.clone(),
            compiled: HashMap::new(),
        }));
        register_helpers(&self.conn, &state);
        let mut stmt = self.conn.prepare(&sql.sql).map_err(|e| format!("prepare: {e}\nSQL: {}", sql.sql))?;
        let width = sql.columns.len().max(1);
        let mut ids: Vec<Vec<Option<i64>>> = Vec::new();
        {
            let mut rows = stmt.query(params_from_iter(sql.params.iter())).map_err(|e| format!("query: {e}"))?;
            while let Some(row) = rows.next().map_err(|e| format!("step: {e}\nSQL: {}", sql.sql))? {
                ids.push((0..width).map(|i| row.get::<_, Option<i64>>(i)).collect::<Result<_, _>>().map_err(|e| e.to_string())?);
            }
        }
        if matches!(lowered.form, sq_core::ir::QueryForm::Ask) {
            return Ok(Outcome::Boolean(!ids.is_empty()));
        }
        let mut st = lock(&state);
        let rows = ids
            .into_iter()
            .map(|row| {
                row.into_iter().take(sql.columns.len()).map(|id| id.map(|id| st.decode(TermId(id)))).collect()
            })
            .collect();
        Ok(Outcome::Solutions { vars: sql.columns.clone(), rows })
    }

    /// spareval's answer on the reference dataset.
    pub fn reference(&self, query: &spargebra::Query) -> Result<Outcome, String> {
        let results = spareval::QueryEvaluator::new().prepare(query).execute(&self.dataset).map_err(|e| e.to_string())?;
        match results {
            spareval::QueryResults::Boolean(b) => Ok(Outcome::Boolean(b)),
            spareval::QueryResults::Solutions(sols) => {
                let vars: Vec<String> = sols.variables().iter().map(|v| v.as_str().to_owned()).collect();
                let mut rows = Vec::new();
                for s in sols {
                    let s = s.map_err(|e| e.to_string())?;
                    rows.push(vars.iter().map(|v| s.get(v.as_str()).cloned()).collect());
                }
                Ok(Outcome::Solutions { vars, rows })
            }
            spareval::QueryResults::Graph(_) => Err("graph results are not compared".into()),
        }
    }
}

/// How two results must agree.
#[derive(Clone, Debug, Default)]
pub struct Expect {
    /// Projected variables whose sequence must match (ORDER BY keys that are plain variables).
    pub order_keys: Option<Vec<String>>,
    /// Compare as sets (REDUCED / mf:LaxCardinality).
    pub lax: bool,
}

impl Expect {
    /// Derives the comparison from the query's algebra.
    pub fn of(query: &spargebra::Query) -> Expect {
        use spargebra::algebra::{Expression, GraphPattern, OrderExpression};
        let spargebra::Query::Select { pattern, .. } = query else { return Expect::default() };
        let mut p = pattern;
        let mut lax = false;
        loop {
            match p {
                GraphPattern::Slice { inner, .. } | GraphPattern::Distinct { inner } => p = inner,
                GraphPattern::Reduced { inner } => {
                    lax = true;
                    p = inner
                }
                GraphPattern::Project { inner, .. } => p = inner,
                _ => break,
            }
        }
        let order_keys = match p {
            GraphPattern::OrderBy { expression, .. } => Some(
                expression
                    .iter()
                    .map_while(|k| match k {
                        OrderExpression::Asc(Expression::Variable(v)) | OrderExpression::Desc(Expression::Variable(v)) => {
                            Some(v.as_str().to_owned())
                        }
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        };
        Expect { order_keys, lax }
    }
}

/// Literal comparison key: numerics by datatype and value (spareval canonicalizes numeric
/// lexical forms on load, the store keeps them), everything else by N-Triples.
fn term_key(t: &oxrdf::Term) -> String {
    if let oxrdf::Term::Literal(l) = t {
        let dt = l.datatype().as_str();
        let numeric = dt.strip_prefix(codec::XSD).is_some_and(|local| {
            matches!(
                local,
                "integer" | "decimal" | "double" | "float" | "int" | "long" | "short" | "byte" | "nonNegativeInteger"
                    | "positiveInteger" | "negativeInteger" | "nonPositiveInteger" | "unsignedLong" | "unsignedInt"
                    | "unsignedShort" | "unsignedByte"
            )
        });
        if numeric && let Ok(v) = l.value().trim().parse::<f64>() {
            return format!("{v:?}^^{dt}");
        }
        if dt == RDF_LANG_STRING_IRI {
            return t.to_string();
        }
    }
    t.to_string()
}

fn row_key(row: &[Option<oxrdf::Term>]) -> Vec<Option<String>> {
    row.iter().map(|t| t.as_ref().map(term_key)).collect()
}

fn has_bnode(row: &[Option<oxrdf::Term>]) -> bool {
    row.iter().any(|t| matches!(t, Some(oxrdf::Term::BlankNode(_))))
}

/// Can `a` map onto `b` under the bnode bijection `map` (extended in place; returns the
/// labels added so a failed branch can undo them)?
fn match_row(
    a: &[Option<oxrdf::Term>],
    b: &[Option<oxrdf::Term>],
    map: &mut HashMap<String, String>,
    rev: &mut HashMap<String, String>,
) -> Option<Vec<String>> {
    let mut added = Vec::new();
    let undo = |added: &[String], map: &mut HashMap<String, String>, rev: &mut HashMap<String, String>| {
        for k in added {
            if let Some(v) = map.remove(k) {
                rev.remove(&v);
            }
        }
    };
    for (x, y) in a.iter().zip(b) {
        let ok = match (x, y) {
            (Some(oxrdf::Term::BlankNode(x)), Some(oxrdf::Term::BlankNode(y))) => {
                match (map.get(x.as_str()), rev.get(y.as_str())) {
                    (Some(m), _) => m == y.as_str(),
                    (None, Some(_)) => false,
                    (None, None) => {
                        map.insert(x.as_str().to_owned(), y.as_str().to_owned());
                        rev.insert(y.as_str().to_owned(), x.as_str().to_owned());
                        added.push(x.as_str().to_owned());
                        true
                    }
                }
            }
            (Some(x), Some(y)) => term_key(x) == term_key(y),
            (None, None) => true,
            _ => false,
        };
        if !ok {
            undo(&added, map, rev);
            return None;
        }
    }
    Some(added)
}

/// Multiset equality of rows under one result-wide bnode bijection (backtracking).
fn same_bag(a: &[Vec<Option<oxrdf::Term>>], b: &[Vec<Option<oxrdf::Term>>]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    if !a.iter().any(|r| has_bnode(r)) && !b.iter().any(|r| has_bnode(r)) {
        let mut ka: Vec<_> = a.iter().map(|r| row_key(r)).collect();
        let mut kb: Vec<_> = b.iter().map(|r| row_key(r)).collect();
        ka.sort();
        kb.sort();
        return ka == kb;
    }
    fn go(
        i: usize,
        a: &[Vec<Option<oxrdf::Term>>],
        b: &[Vec<Option<oxrdf::Term>>],
        used: &mut [bool],
        map: &mut HashMap<String, String>,
        rev: &mut HashMap<String, String>,
    ) -> bool {
        if i == a.len() {
            return true;
        }
        for j in 0..b.len() {
            if used[j] {
                continue;
            }
            if let Some(added) = match_row(&a[i], &b[j], map, rev) {
                used[j] = true;
                if go(i + 1, a, b, used, map, rev) {
                    return true;
                }
                used[j] = false;
                for k in added {
                    if let Some(v) = map.remove(&k) {
                        rev.remove(&v);
                    }
                }
            }
        }
        false
    }
    go(0, a, b, &mut vec![false; b.len()], &mut HashMap::new(), &mut HashMap::new())
}

fn dedup(rows: &[Vec<Option<oxrdf::Term>>]) -> Vec<Vec<Option<oxrdf::Term>>> {
    let mut seen = std::collections::HashSet::new();
    rows.iter().filter(|r| seen.insert(row_key(r))).cloned().collect()
}

/// Compares the baseline's outcome with the reference.
pub fn compare(ours: &Outcome, reference: &Outcome, expect: &Expect) -> Result<(), String> {
    match (ours, reference) {
        (Outcome::Boolean(a), Outcome::Boolean(b)) if a == b => Ok(()),
        (Outcome::Solutions { vars: va, rows: ra }, Outcome::Solutions { vars: vb, rows: rb }) => {
            // Align the reference's columns with ours by name.
            let mut sa: Vec<&String> = va.iter().collect();
            let mut sb: Vec<&String> = vb.iter().collect();
            sa.sort();
            sb.sort();
            if sa != sb {
                return Err(format!("variables differ: {va:?} vs {vb:?}"));
            }
            let idx: Vec<usize> = va.iter().map(|v| vb.iter().position(|w| w == v).expect("same set")).collect();
            let rb: Vec<Vec<Option<oxrdf::Term>>> =
                rb.iter().map(|r| idx.iter().map(|i| r[*i].clone()).collect()).collect();
            let equal = if expect.lax { same_bag(&dedup(ra), &dedup(&rb)) } else { same_bag(ra, &rb) };
            if !equal {
                return Err(format!("solutions differ:\n ours ({}): {}\n reference ({}): {}", ra.len(), show(ra), rb.len(), show(&rb)));
            }
            if let Some(keys) = &expect.order_keys {
                let ki: Vec<usize> = keys.iter().filter_map(|k| va.iter().position(|v| v == k)).collect();
                let proj = |rows: &[Vec<Option<oxrdf::Term>>]| -> Vec<Vec<Option<String>>> {
                    rows.iter().map(|r| ki.iter().map(|i| r[*i].as_ref().map(term_key)).collect()).collect()
                };
                // Blank-node keys cannot be compared by label.
                if !ra.iter().any(|r| has_bnode(r)) && proj(ra) != proj(&rb) {
                    return Err(format!("order differs:\n ours: {}\n reference: {}", show(ra), show(&rb)));
                }
            }
            Ok(())
        }
        _ => Err(format!("different result kinds: {ours:?} vs {reference:?}")),
    }
}

fn show(rows: &[Vec<Option<oxrdf::Term>>]) -> String {
    const MAX: usize = 12;
    let mut out: Vec<String> = rows
        .iter()
        .take(MAX)
        .map(|r| {
            let cells: Vec<String> = r.iter().map(|t| t.as_ref().map_or("UNDEF".to_owned(), |t| t.to_string())).collect();
            format!("({})", cells.join(" "))
        })
        .collect();
    if rows.len() > MAX {
        out.push(format!("… {} more", rows.len() - MAX));
    }
    out.join(" ")
}

/// Runs a query on both engines; Ok(None) when the baseline does not support it.
pub fn check(engine: &Engine, query_text: &str, base: Option<&str>) -> Result<Option<SqlQuery>, String> {
    let query = sq_plan::parse_query(query_text, base).map_err(|e| e.to_string())?;
    match engine.baseline(&query)? {
        Baseline::Unsupported(_) => Ok(None),
        Baseline::Ran { sql, outcome } => {
            let reference = engine.reference(&query)?;
            compare(&outcome, &reference, &Expect::of(&query)).map_err(|e| format!("{e}\nSQL: {}", sql.sql))?;
            Ok(Some(sql))
        }
    }
}
