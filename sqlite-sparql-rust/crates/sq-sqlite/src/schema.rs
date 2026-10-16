//! §2.1 DDL (schema_version 1, encoding_version 1) and schema-state detection.
use rustc_hash::FxHashSet;
use sq_core::codec::{ENCODING_VERSION, SCHEMA_VERSION};
use sq_core::{Result, Tag};

use crate::raw::{RawDb, RawStmt};
use crate::shared::StorageShared;

/// Tables, created in this order by `sparql_init()`.
pub(crate) const TABLES_DDL: &str = "
CREATE TABLE IF NOT EXISTS main.rdf_meta(key TEXT PRIMARY KEY, value ANY) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS main.rdf_term(
  id   INTEGER PRIMARY KEY,
  tag  INTEGER NOT NULL CHECK (tag IN (1,3,4,5)),
  lex  TEXT    NOT NULL,
  dt   INTEGER,
  lang TEXT,
  h    INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS main.rdf_quad(
  g INTEGER NOT NULL,
  s INTEGER NOT NULL, p INTEGER NOT NULL, o INTEGER NOT NULL,
  PRIMARY KEY (g, s, p, o)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS main.rdf_graph(g INTEGER PRIMARY KEY);
CREATE TABLE IF NOT EXISTS main.rdf_stat_global(scope INTEGER PRIMARY KEY, n INTEGER NOT NULL,
  ds INTEGER NOT NULL, dp INTEGER NOT NULL, dobj INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS main.rdf_stat_pred(scope INTEGER NOT NULL, p INTEGER NOT NULL,
  n INTEGER NOT NULL, ds INTEGER NOT NULL, dobj INTEGER NOT NULL, imin INTEGER, imax INTEGER,
  PRIMARY KEY (scope, p)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS main.rdf_stat_po(scope INTEGER NOT NULL, p INTEGER NOT NULL,
  o INTEGER NOT NULL, n INTEGER NOT NULL, PRIMARY KEY (scope, p, o)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS main.rdf_stat_cs(scope INTEGER NOT NULL, cs INTEGER NOT NULL,
  subjects INTEGER NOT NULL, preds BLOB NOT NULL, occ BLOB NOT NULL,
  PRIMARY KEY (scope, cs)) WITHOUT ROWID;
";

/// Hash index of the dictionary (not UNIQUE: collisions are resolved by exact compare). The
/// loader drops it while it appends to a dictionary that needs no probing.
pub(crate) const TERM_HASH_INDEX_DDL: &str = "CREATE INDEX IF NOT EXISTS main.rdf_term_h ON rdf_term(h)";

/// `rdf_meta` defaults (INSERT OR IGNORE: existing values are kept).
pub(crate) fn meta_defaults_sql() -> String {
    format!(
        "INSERT OR IGNORE INTO main.rdf_meta(key, value) VALUES
  ('schema_version', {SCHEMA_VERSION}), ('encoding_version', {ENCODING_VERSION}),
  ('next_bnode', 1), ('write_gen', 0), ('dict_gen', 0), ('stats_gen', 0),
  ('analyzed_quads', 0), ('created_by', 'sparqlite {}'), ('db_id', random());",
        env!("CARGO_PKG_VERSION")
    )
}

/// The 8 secondary quad indexes (name, DDL). The bulk loader drops and recreates exactly
/// these.
pub(crate) const QUAD_INDEXES: [(&str, &str); 8] = [
    ("rdf_gpso", "CREATE INDEX IF NOT EXISTS main.rdf_gpso ON rdf_quad(g, p, s, o)"),
    ("rdf_gpos", "CREATE INDEX IF NOT EXISTS main.rdf_gpos ON rdf_quad(g, p, o, s)"),
    ("rdf_gosp", "CREATE INDEX IF NOT EXISTS main.rdf_gosp ON rdf_quad(g, o, s, p)"),
    ("rdf_spog", "CREATE INDEX IF NOT EXISTS main.rdf_spog ON rdf_quad(s, p, o, g) WHERE g<>0"),
    ("rdf_psog", "CREATE INDEX IF NOT EXISTS main.rdf_psog ON rdf_quad(p, s, o, g) WHERE g<>0"),
    ("rdf_posg", "CREATE INDEX IF NOT EXISTS main.rdf_posg ON rdf_quad(p, o, s, g) WHERE g<>0"),
    ("rdf_ospg", "CREATE INDEX IF NOT EXISTS main.rdf_ospg ON rdf_quad(o, s, p, g) WHERE g<>0"),
    ("rdf_gpox", "CREATE INDEX IF NOT EXISTS main.rdf_gpox ON rdf_quad(g, p, o, s) WHERE (o & 15) IN (5,8)"),
];

/// Automatic index of rdf_quad's PRIMARY KEY (g,s,p,o) (the WITHOUT ROWID table B-tree).
pub(crate) const PK_INDEX: &str = "sqlite_autoindex_rdf_quad_1";
/// A PRAGMA statement (rows in seqno order: seqno, cid, name), not the `pragma_index_info`
/// table-valued function: SQLite 3.45's eponymous pragma tables keep a pointer to the main
/// schema they were first used with, and after `sqlite3_deserialize` replaces main, using
/// such a table again can crash inside SQLite. Detection runs after every main-file change.
const PK_COLUMNS_SQL: &str = "PRAGMA main.index_info('sqlite_autoindex_rdf_quad_1')";

/// Tables whose absence makes the store incompatible (they hold the data).
pub(crate) const CORE_TABLES: [&str; 3] = ["rdf_meta", "rdf_term", "rdf_quad"];
/// The registry and statistics tables: re-created by `ensure_schema` like a missing index.
pub(crate) const AUX_TABLES: [&str; 5] = ["rdf_graph", "rdf_stat_global", "rdf_stat_pred", "rdf_stat_po", "rdf_stat_cs"];

/// Public schema state (§4.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaState {
    Absent,
    Ready { schema_version: i64 },
    Incompatible(String),
}

/// Detected state plus whether every auxiliary table and index exists (else `ensure_schema`
/// re-runs the idempotent DDL).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Detected {
    pub state: SchemaState,
    pub complete: bool,
}

/// Reads `sqlite_schema` and the version rows (one-shot statements; runs only when the
/// schema cookie changed).
pub(crate) fn detect(db: RawDb) -> Result<Detected> {
    let mut names: FxHashSet<String> = FxHashSet::default();
    let mut st = RawStmt::prepare(db, "SELECT name FROM main.sqlite_schema WHERE name GLOB 'rdf_*'", false)?;
    while st.step()? {
        names.insert(st.col_str(0)?.to_string());
    }
    drop(st);
    if !names.contains("rdf_meta") {
        if names.contains("rdf_quad") || names.contains("rdf_term") {
            let state = SchemaState::Incompatible("rdf_quad/rdf_term exist but rdf_meta is missing".into());
            return Ok(Detected { state, complete: false });
        }
        return Ok(Detected { state: SchemaState::Absent, complete: false });
    }
    let mut versions = [None, None];
    let mut st = RawStmt::prepare(
        db,
        "SELECT key, value FROM main.rdf_meta WHERE key IN ('schema_version','encoding_version')",
        false,
    )?;
    while st.step()? {
        let v = st.col_opt_i64(1);
        match st.col_str(0)? {
            "schema_version" => versions[0] = v,
            _ => versions[1] = v,
        }
    }
    drop(st);
    let incompatible = |msg: String| Ok(Detected { state: SchemaState::Incompatible(msg), complete: false });
    if versions[0] != Some(SCHEMA_VERSION) {
        return incompatible(format!("schema_version {:?}, expected {SCHEMA_VERSION}", versions[0]));
    }
    if versions[1] != Some(ENCODING_VERSION) {
        return incompatible(format!("encoding_version {:?}, expected {ENCODING_VERSION}", versions[1]));
    }
    if let Some(missing) = CORE_TABLES.iter().find(|t| !names.contains(**t)) {
        return incompatible(format!("table {missing} is missing"));
    }
    // The GSPO templates pin the primary key by its automatic index name (template::PK_FROM);
    // WITHOUT ROWID primary keys are not listed in sqlite_schema, so ask PRAGMA index_info.
    let mut pk = Vec::new();
    let mut st = RawStmt::prepare(db, PK_COLUMNS_SQL, false)?;
    while st.step()? {
        pk.push(st.col_str(2)?.to_string());
    }
    drop(st);
    if pk != ["g", "s", "p", "o"] {
        return incompatible(format!("rdf_quad primary key {PK_INDEX} is {pk:?}, expected (g, s, p, o)"));
    }
    let complete = names.contains("rdf_term_h")
        && QUAD_INDEXES.iter().all(|(n, _)| names.contains(*n))
        && AUX_TABLES.iter().all(|t| names.contains(*t));
    Ok(Detected { state: SchemaState::Ready { schema_version: SCHEMA_VERSION }, complete })
}

/// Runs the full idempotent DDL plus the WELL_KNOWN rows. The caller holds a write scope; the
/// first CREATE TABLE takes its write lock. `CREATE INDEX` expires every prepared statement,
/// so the scope's forced-rollback statement is refreshed after the DDL (`scope::Abort`).
pub(crate) fn create(shared: &StorageShared) -> Result<()> {
    let db = shared.db();
    db.exec(TABLES_DDL)?;
    shared.refresh_abort()?;
    db.exec(TERM_HASH_INDEX_DDL)?;
    for (_, ddl) in QUAD_INDEXES {
        db.exec(ddl)?;
    }
    shared.refresh_abort()?;
    db.exec(&meta_defaults_sql())?;
    let mut ins = RawStmt::prepare(
        db,
        "INSERT OR IGNORE INTO main.rdf_term(id,tag,lex,dt,lang,h) VALUES (?1,1,?2,NULL,NULL,?3)",
        false,
    )?;
    for (row, iri) in sq_core::codec::WELL_KNOWN {
        ins.reset();
        ins.bind_i64(1, *row)?;
        ins.bind_text(2, iri)?;
        ins.bind_i64(3, sq_core::codec::term_hash(Tag::Iri, iri, ""))?;
        ins.run()?;
    }
    Ok(())
}
