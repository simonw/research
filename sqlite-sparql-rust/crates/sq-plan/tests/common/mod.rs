//! Shared helpers for the sq-plan integration tests.
#![allow(dead_code)]
use rustc_hash::FxHashMap;
use sq_core::ir::LoweredQuery;
use sq_core::plan::{Op, Plan, QueryOptions, QueryPlan};
use sq_core::store::QuadStoreWriter;
use sq_core::TermId;
use sq_memstore::MemStore;

pub const PREFIXES: &str = "PREFIX : <http://ex/>\nPREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>\n";

/// Load Turtle into `graph` (None = default graph); blank nodes are fresh per call.
pub fn load_ttl(store: &MemStore, ttl: &str, graph: Option<&str>) {
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
    let s = store(&ttl);
    s.analyze();
    s
}
