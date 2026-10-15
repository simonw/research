//! Shared helpers for the sq-plan integration tests.
#![allow(dead_code)]
use std::rc::Rc;
use std::sync::Arc;

use rustc_hash::FxHashMap;
use sq_core::eval_api::SolutionSource;
use sq_core::ir::LoweredQuery;
use sq_core::plan::{Op, Plan, QueryOptions, QueryPlan};
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_core::{Term, TermId};
use sq_exec::{ExecCaches, Execution};
use sq_memstore::MemStore;
use sq_sqlite::{RawDb, SqliteStore, StorageConfig, StorageShared};

pub const PREFIXES: &str = "PREFIX : <http://ex/>\nPREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>\n";

/// Load Turtle into `graph` (None = default graph); blank nodes are fresh per call.
pub fn load_ttl(store: &MemStore, ttl: &str, graph: Option<&str>) {
    load_ttl_into(store, ttl, graph)
}

/// [`load_ttl`] into any writable store.
pub fn load_ttl_into(store: &dyn QuadStoreWriter, ttl: &str, graph: Option<&str>) {
    let g = graph.map_or(TermId::DEFAULT_GRAPH, |g| store.intern(oxrdf::NamedNode::new_unchecked(g).as_ref().into()).unwrap());
    let mut bnodes: FxHashMap<String, TermId> = FxHashMap::default();
    let mut quads = Vec::new();
    for t in oxttl::TurtleParser::new().for_slice(ttl.as_bytes()) {
        let t = t.expect("valid turtle");
        let mut id = |x: oxrdf::TermRef<'_>| match x {
            oxrdf::TermRef::BlankNode(b) => {
                *bnodes.entry(b.as_str().to_owned()).or_insert_with(|| store.fresh_bnode().unwrap())
            }
            other => store.intern(other).unwrap(),
        };
        quads.push([g, id(t.subject.as_ref().into()), id(t.predicate.as_ref().into()), id(t.object.as_ref())]);
    }
    store.insert(&mut quads).unwrap();
}

pub fn store(ttl: &str) -> MemStore {
    let s = MemStore::new();
    load_ttl(&s, &format!("@prefix : <http://ex/> .\n{ttl}"), None);
    s
}

pub fn lower(q: &str, store: &MemStore) -> LoweredQuery {
    lower_with(q, store, &QueryOptions::default())
}

pub fn lower_with(q: &str, store: &MemStore, opts: &QueryOptions) -> LoweredQuery {
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).expect("query parses");
    sq_plan::lower_query(&parsed, store, opts).expect("query lowers")
}

pub fn plan(q: &str, store: &MemStore) -> QueryPlan {
    plan_with(q, store, &QueryOptions::default())
}

pub fn plan_with(q: &str, store: &MemStore, opts: &QueryOptions) -> QueryPlan {
    let lowered = lower_with(q, store, opts);
    let p = sq_plan::optimize(lowered, store, opts).expect("query plans");
    sq_plan::validate(&p).expect("plan validates");
    p
}

/// Every node of the root plan and the EXISTS plans, pre-order.
pub fn nodes(p: &QueryPlan) -> Vec<&Plan> {
    fn walk<'a>(p: &'a Plan, out: &mut Vec<&'a Plan>) {
        out.push(p);
        for c in p.children() {
            walk(c, out);
        }
    }
    let mut out = Vec::new();
    walk(&p.root, &mut out);
    for e in &p.exists {
        walk(e, &mut out);
    }
    out
}

pub fn count_ops(p: &QueryPlan, pred: impl Fn(&Op) -> bool) -> usize {
    nodes(p).into_iter().filter(|n| pred(&n.op)).count()
}

/// Slot of a variable by name (first user / renamed slot with that name).
pub fn slot(q: &QueryPlan, name: &str) -> u16 {
    q.vars.vars.iter().position(|(n, _)| n == name).unwrap_or_else(|| panic!("no variable {name}")) as u16
}

/// A deterministic "social" graph: N persons with type, age, name, city and 3 knows edges.
pub fn social(n: usize) -> MemStore {
    let s = store(&social_ttl(n));
    s.analyze();
    s
}

/// The same graph in an in-memory SQLite store, analyzed.
pub fn sqlite_social(n: usize) -> Sqlite {
    let db = Sqlite::new();
    db.load(&format!("@prefix : <http://ex/> .\n{}", social_ttl(n)), None);
    db.store.analyze().unwrap();
    db
}

/// Turtle of [`social`].
pub fn social_ttl(n: usize) -> String {
    let mut ttl = String::new();
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for i in 0..n {
        ttl.push_str(&format!(
            ":p{i} a :C{} ; :age {} ; :name \"Person {i}\" ; :city :city{} .\n",
            i % 10,
            next() % 100,
            i % 20
        ));
        for _ in 0..3 {
            ttl.push_str(&format!(":p{i} :knows :p{} .\n", next() % n as u64));
        }
    }
    ttl
}

/// An in-memory SQLite store (schema initialized, statement pool armed) on its own
/// connection. Fields drop in order: the store, then the connection; executions holding the
/// store must be dropped before it.
pub struct Sqlite {
    pub store: SqliteStore,
    pub conn: rusqlite::Connection,
}

impl Sqlite {
    pub fn new() -> Sqlite {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory database");
        // SAFETY: the connection outlives the store (field order) and every user of it.
        let db = unsafe { RawDb::from_ptr(conn.handle()) };
        let shared = StorageShared::new(db, StorageConfig::default());
        shared.arm_pool();
        let store = SqliteStore::new(shared);
        store.ensure_schema().expect("schema");
        Sqlite { store, conn }
    }

    /// Load Turtle in one committed write scope.
    pub fn load(&self, ttl: &str, graph: Option<&str>) {
        let scope = self.store.write_scope(0).expect("write scope");
        load_ttl_into(&self.store, ttl, graph);
        scope.commit().expect("commit");
    }
}

/// A solution with its terms decoded (None = unbound).
pub type Solution = Vec<Option<Arc<Term>>>;

/// The solutions of `plan`, executed by sq-exec on `store`, sorted (a multiset). A SELECT
/// without variables returns no ids, so its solutions are counted from `next_rows`.
pub fn execute(store: Rc<dyn QuadStore>, plan: QueryPlan, opts: &QueryOptions) -> Result<Vec<Solution>, String> {
    let mut ex = Execution::new(store, plan, opts, ExecCaches::default()).map_err(|e| e.to_string())?;
    let width = ex.variables().len();
    let (mut ids, mut produced) = (Vec::new(), 0);
    loop {
        let n = ex.next_rows(&mut ids, 1024).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        produced += n;
    }
    let mut terms = Vec::with_capacity(ids.len());
    ex.decode(&ids, &mut terms).map_err(|e| e.to_string())?;
    let mut rows: Vec<Solution> = if width == 0 {
        vec![Vec::new(); produced]
    } else {
        terms.chunks(width).map(<[_]>::to_vec).collect()
    };
    rows.sort_by_cached_key(|r| format!("{r:?}"));
    Ok(rows)
}

/// The oracle's solutions of a lowered SELECT query, sorted (a multiset).
pub fn oracle(q: &LoweredQuery, store: &dyn QuadStore) -> Result<Vec<Solution>, String> {
    match sq_oracle::evaluate(q, store).map_err(|e| e.to_string())? {
        sq_oracle::OracleOutput::Solutions { mut rows, .. } => {
            rows.sort_by_cached_key(|r| format!("{r:?}"));
            Ok(rows)
        }
        other => Err(format!("not a SELECT result: {other:?}")),
    }
}

/// Lower `q` (with the test prefixes) against any store.
pub fn lower_on(q: &str, store: &dyn QuadStore, opts: &QueryOptions) -> LoweredQuery {
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).expect("query parses");
    sq_plan::lower_query(&parsed, store, opts).expect("query lowers")
}

/// Lower, optimize and validate `q` against any store.
pub fn plan_on(q: &str, store: &dyn QuadStore, opts: &QueryOptions) -> QueryPlan {
    let p = sq_plan::optimize(lower_on(q, store, opts), store, opts).expect("query plans");
    sq_plan::validate(&p).expect("plan validates");
    p
}
