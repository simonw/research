//! The engines under comparison: sq-oracle (ground truth) and the native engine (sq-plan +
//! sq-exec through `sq_engine::prepare`) on a MemStore and on an in-memory SQLite store
//! holding the same data.
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use sq_core::eval_api::{SolutionSource, TripleSource};
use sq_core::ir::QueryForm;
use sq_core::plan::{JoinPolicy, QueryOptions};
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_core::Result;
use sq_exec::ExecCaches;
use sq_memstore::MemStore;
use sq_oracle::{OracleOptions, OracleOutput};
use sq_sqlite::{RawDb, SqliteStore, StorageConfig, StorageShared};

use super::compare::{Outcome, Row};
use super::data::{Dataset, EMPTY_GRAPH};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StoreKind {
    Mem,
    /// In-memory SQLite: SQL cursors.
    Sqlite,
    /// A rollback-journal SQLite file with 512-byte pages: inside a read transaction its
    /// cursors are the direct B-tree page reader, and even small datasets give multi-level
    /// B-trees.
    File,
}

impl StoreKind {
    pub const ALL: [StoreKind; 3] = [StoreKind::Mem, StoreKind::Sqlite, StoreKind::File];
}

/// One native configuration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub store: StoreKind,
    pub policy: JoinPolicy,
    pub reorder: bool,
}

impl Config {
    /// Every store x the given policies x reorder on / off.
    pub fn all(policies: &[JoinPolicy]) -> Vec<Config> {
        let mut out = Vec::new();
        for store in StoreKind::ALL {
            for &policy in policies {
                for reorder in [true, false] {
                    out.push(Config { store, policy, reorder });
                }
            }
        }
        out
    }
}

/// A SQLite store on its own connection. Fields drop in order: the store, then the
/// connection, then the database file (if any).
pub struct SqliteDb {
    pub store: SqliteStore,
    /// Owns the database the store points into.
    _conn: rusqlite::Connection,
    file: Option<TempFile>,
}

/// A database file removed on drop.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        for suffix in ["", "-journal"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.0.display()));
        }
    }
}

static FILES: AtomicU64 = AtomicU64::new(0);

impl SqliteDb {
    fn open(file: Option<TempFile>) -> SqliteDb {
        let conn = match &file {
            Some(f) => {
                let conn = rusqlite::Connection::open(&f.0).expect("database file");
                conn.execute_batch("PRAGMA page_size=512; PRAGMA journal_mode=DELETE; PRAGMA synchronous=OFF;").expect("pragmas");
                conn
            }
            None => rusqlite::Connection::open_in_memory().expect("in-memory database"),
        };
        // SAFETY: the connection outlives the store (field order) and every user of it.
        let db = unsafe { RawDb::from_ptr(conn.handle()) };
        let shared = StorageShared::new(db, StorageConfig::default());
        shared.arm_pool();
        let store = SqliteStore::new(shared);
        store.ensure_schema().expect("schema");
        SqliteDb { store, _conn: conn, file }
    }

    fn memory() -> SqliteDb {
        SqliteDb::open(None)
    }

    fn file() -> SqliteDb {
        let n = FILES.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("sq-engine-fuzz-{}-{n}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        SqliteDb::open(Some(TempFile(path)))
    }

    fn load(&self, trig: &str, empty_graph: bool, analyze: bool) {
        let scope = self.store.write_scope(0).expect("write scope");
        sq_engine::load(&self.store, trig.as_bytes(), oxrdfio::RdfFormat::TriG, None, None).expect("dataset loads");
        if empty_graph {
            register_empty_graph(&self.store);
        }
        if analyze {
            self.store.analyze().expect("analyze");
        }
        scope.commit().expect("commit");
    }

    /// Runs `f` inside a read scope after the epoch check, as sq-api does.
    fn read<R>(&self, f: impl FnOnce(Rc<dyn QuadStore>) -> R) -> R {
        let scope = self.store.read_scope(0).expect("read scope");
        self.store.shared().validate_epoch().expect("epoch");
        if self.file.is_some() {
            assert!(self.store.shared().btree_active(), "file stores are read by the B-tree reader");
        }
        let r = f(Rc::new(self.store.clone()));
        scope.close().expect("read scope closes");
        r
    }
}

impl Drop for SqliteDb {
    fn drop(&mut self) {
        self.store.shared().disarm_and_finalize();
    }
}

/// The same dataset in every store.
pub struct Stores {
    pub mem: MemStore,
    pub sqlite: SqliteDb,
    pub file: SqliteDb,
}

/// Registers the dataset's empty named graph.
fn register_empty_graph(store: &dyn QuadStoreWriter) {
    let iri = format!("http://ex/{EMPTY_GRAPH}");
    let id = store.intern(oxrdf::NamedNodeRef::new_unchecked(&iri).into()).expect("graph IRI interns");
    store.create_graph(id).expect("graph registers");
}

/// `data` in a MemStore.
pub fn mem_store(data: &Dataset) -> MemStore {
    let mem = MemStore::new();
    sq_engine::load(&mem, data.trig().as_bytes(), oxrdfio::RdfFormat::TriG, None, None).expect("dataset loads");
    if data.empty_graph {
        register_empty_graph(&mem);
    }
    if data.analyze {
        mem.analyze();
    }
    mem
}

impl Stores {
    pub fn load(data: &Dataset) -> Stores {
        let trig = data.trig();
        let mem = mem_store(data);
        let sqlite = SqliteDb::memory();
        sqlite.load(&trig, data.empty_graph, data.analyze);
        let file = SqliteDb::file();
        file.load(&trig, data.empty_graph, data.analyze);
        Stores { mem, sqlite, file }
    }

    /// Runs `f` against one store.
    pub fn with<R>(&self, kind: StoreKind, f: impl FnOnce(Rc<dyn QuadStore>) -> R) -> R {
        match kind {
            StoreKind::Mem => f(Rc::new(self.mem.clone())),
            StoreKind::Sqlite => self.sqlite.read(f),
            StoreKind::File => self.file.read(f),
        }
    }
}

pub fn now() -> oxsdatatypes::DateTime {
    oxsdatatypes::DateTime::from_str("2026-10-15T12:00:00Z").unwrap()
}

/// Ground truth.
pub fn oracle(store: &dyn QuadStore, q: &spargebra::Query, opts: &QueryOptions) -> Outcome {
    let run = || -> Result<OracleOutput> {
        let lowered = sq_plan::lower_query(q, store, opts)?;
        sq_oracle::evaluate_with(&lowered, store, &OracleOptions { now: Some(now()), rng_seed: Some(1) })
    };
    match run() {
        Ok(OracleOutput::Solutions { vars, rows }) => Outcome::Solutions { vars, rows },
        Ok(OracleOutput::Boolean(b)) => Outcome::Boolean(b),
        Ok(OracleOutput::Graph(triples)) => {
            Outcome::Graph(triples.into_iter().map(|t| t.into_iter().map(Some).collect()).collect())
        }
        Err(e) => Outcome::Error(e.to_string()),
    }
}

thread_local! {
    /// Harness self-test: the native engine "loses" the last row of any answer of 2+ rows.
    static SABOTAGE: Cell<bool> = const { Cell::new(false) };
}

/// Turns the self-test sabotage on or off for this thread.
pub fn set_sabotage(on: bool) {
    SABOTAGE.with(|s| s.set(on));
}

/// The native engine.
pub fn native(store: Rc<dyn QuadStore>, q: &spargebra::Query, opts: &QueryOptions) -> Outcome {
    match run_native(store, q, opts) {
        Ok(Outcome::Solutions { vars, mut rows }) if SABOTAGE.with(Cell::get) && rows.len() >= 2 => {
            rows.pop();
            Outcome::Solutions { vars, rows }
        }
        Ok(o) => o,
        Err(e) => Outcome::Error(e.to_string()),
    }
}

fn run_native(store: Rc<dyn QuadStore>, q: &spargebra::Query, opts: &QueryOptions) -> Result<Outcome> {
    let mut exec = sq_engine::prepare(store, q, opts, ExecCaches::default())?;
    exec.set_now(now());
    match exec.form().clone() {
        QueryForm::Ask => Ok(Outcome::Boolean(exec.ask()?)),
        QueryForm::Select(_) => {
            let vars = exec.variables().to_vec();
            let width = vars.len();
            let mut ids = Vec::new();
            let mut n = 0;
            loop {
                let got = exec.next_rows(&mut ids, 1024)?;
                if got == 0 {
                    break;
                }
                n += got;
            }
            let mut terms = Vec::with_capacity(ids.len());
            SolutionSource::decode(&mut exec, &ids, &mut terms)?;
            let rows: Vec<Row> = if width == 0 { vec![Vec::new(); n] } else { terms.chunks(width).map(<[_]>::to_vec).collect() };
            Ok(Outcome::Solutions { vars, rows })
        }
        QueryForm::Construct(_) | QueryForm::Describe(_) => {
            let mut triples = Vec::new();
            loop {
                let got = exec.next_triples(&mut triples, 1024)?;
                if got == 0 {
                    break;
                }
            }
            let ids: Vec<_> = triples.iter().flatten().copied().collect();
            let mut terms = Vec::with_capacity(ids.len());
            TripleSource::decode(&mut exec, &ids, &mut terms)?;
            Ok(Outcome::Graph(terms.chunks(3).map(<[_]>::to_vec).collect()))
        }
    }
}

/// Query options of a configuration.
pub fn options(cfg: &Config, union_default: bool, probes: bool) -> QueryOptions {
    QueryOptions {
        join_policy: cfg.policy,
        reorder: cfg.reorder,
        union_default_graph: union_default,
        timeout_ms: Some(60_000),
        max_probe_counts: if probes { QueryOptions::default().max_probe_counts } else { 0 },
        ..QueryOptions::default()
    }
}

/// Options for the oracle's lowering (the policy does not matter there).
pub fn oracle_options(union_default: bool) -> QueryOptions {
    QueryOptions { union_default_graph: union_default, ..QueryOptions::default() }
}
