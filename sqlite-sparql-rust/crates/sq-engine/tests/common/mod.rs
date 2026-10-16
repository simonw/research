//! Helpers shared by the sq-engine integration tests.
#![allow(dead_code)]
use std::rc::Rc;
use std::str::FromStr;

use sq_core::eval_api::SolutionSource;
use sq_core::plan::QueryOptions;
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_core::{Term, TermId};
use sq_engine::{UpdateOptions, UpdateReport};
use sq_exec::ExecCaches;
use sq_memstore::MemStore;
use sq_sqlite::{RawDb, SqliteStore, StorageConfig, StorageShared};

pub const EX: &str = "http://ex/";
pub const PREFIXES: &str = "PREFIX : <http://ex/>\nPREFIX xsd: <http://www.w3.org/2001/XMLSchema#>\n";

/// A MemStore holding `trig` (prefixes `:` and `xsd:` predeclared).
pub fn mem(trig: &str) -> MemStore {
    let store = MemStore::new();
    load_trig(&store, trig);
    store
}

pub fn load_trig(store: &dyn QuadStoreWriter, trig: &str) {
    let text = format!("@prefix : <{EX}> .\n@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n{trig}");
    sq_engine::load(store, text.as_bytes(), oxrdfio::RdfFormat::TriG, Some(EX), None).expect("valid TriG");
}

pub fn options() -> QueryOptions {
    QueryOptions { base_iri: Some(EX.to_owned()), ..QueryOptions::default() }
}

/// A fixed `NOW()` for reproducible updates.
pub fn fixed_now() -> oxsdatatypes::DateTime {
    oxsdatatypes::DateTime::from_str("2026-10-15T12:00:00Z").unwrap()
}

pub fn update_on(store: Rc<dyn QuadStoreWriter>, text: &str) -> sq_core::Result<UpdateReport> {
    let u = sq_plan::parse_update(&format!("{PREFIXES}{text}"), Some(EX))?;
    let uopts = UpdateOptions { now: Some(fixed_now()), ..UpdateOptions::default() };
    sq_engine::update_with(store, &u, &options(), &uopts)
}

/// Runs an update on a MemStore (which is a handle: clones share the data).
pub fn update(store: &MemStore, text: &str) -> sq_core::Result<UpdateReport> {
    update_on(Rc::new(store.clone()), text)
}

/// `:local` for the test namespace, else N-Triples.
pub fn show(t: &Term) -> String {
    match t {
        Term::Iri(i) if i.starts_with(EX) => format!(":{}", &i[EX.len()..]),
        t => t.to_ntriples(),
    }
}

/// SELECT solutions as sorted `?v=term ...` lines (unbound variables omitted).
pub fn select(store: Rc<dyn QuadStore>, query: &str) -> Vec<String> {
    let q = sq_plan::parse_query(&format!("{PREFIXES}{query}"), Some(EX)).expect("query parses");
    let mut exec = sq_engine::prepare(store, &q, &options(), ExecCaches::default()).expect("query prepares");
    let vars = exec.variables().to_vec();
    let mut ids = Vec::new();
    let mut rows = 0;
    loop {
        let n = exec.next_rows(&mut ids, 1024).unwrap();
        if n == 0 {
            break;
        }
        rows += n;
    }
    let mut terms = Vec::new();
    exec.decode(&ids, &mut terms).unwrap();
    let mut out: Vec<String> = (0..rows)
        .map(|r| {
            let row = &terms[r * vars.len()..(r + 1) * vars.len()];
            let cells: Vec<String> =
                vars.iter().zip(row).filter_map(|(v, t)| Some(format!("?{v}={}", show(t.as_deref()?)))).collect();
            cells.join(" ")
        })
        .collect();
    out.sort();
    out
}

pub fn select_mem(store: &MemStore, query: &str) -> Vec<String> {
    select(Rc::new(store.clone()), query)
}

/// Every quad as a sorted `g s p o` line; store blank nodes as `_:bN` in order of first
/// appearance (so tests do not depend on counter values).
pub fn dump(store: &dyn QuadStore, quads: &[[TermId; 4]]) -> Vec<String> {
    let mut bnodes: Vec<TermId> = Vec::new();
    let mut lines: Vec<String> = quads
        .iter()
        .map(|q| {
            q.iter()
                .map(|id| {
                    if *id == TermId::DEFAULT_GRAPH {
                        "(default)".to_owned()
                    } else if id.is_bnode() {
                        let n = bnodes.iter().position(|b| b == id).unwrap_or_else(|| {
                            bnodes.push(*id);
                            bnodes.len() - 1
                        });
                        format!("_:b{n}")
                    } else {
                        show(&store.dict().decode_one(*id).unwrap())
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    lines.sort();
    lines
}

pub fn dump_mem(store: &MemStore) -> Vec<String> {
    dump(store, &store.quads())
}

/// The distinct blank nodes of a MemStore.
pub fn bnode_count(store: &MemStore) -> usize {
    let mut ids: Vec<TermId> = store.quads().iter().flatten().copied().filter(|id| id.is_bnode()).collect();
    ids.sort();
    ids.dedup();
    ids.len()
}

/// An in-memory SQLite store (schema initialized, pool armed). Fields drop in order: the
/// store, then the connection.
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

    pub fn rc(&self) -> Rc<SqliteStore> {
        Rc::new(self.store.clone())
    }
}

impl Drop for Sqlite {
    fn drop(&mut self) {
        self.store.shared().disarm_and_finalize();
    }
}
