//! Shared fixtures of the sq-api integration tests: connections over the system libsqlite3
//! with `sq_api::register`, and a small social graph.
#![allow(dead_code)]

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension};

/// A small graph: 4 people with names, ages, knows edges; one named graph.
pub const DATA: &str = r#"
@prefix ex: <http://ex/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
ex:alice ex:name "Alice" ; ex:age 30 ; ex:knows ex:bob, ex:carol .
ex:bob ex:name "Bob" ; ex:age 25 ; ex:knows ex:carol .
ex:carol ex:name "Carol"@en ; ex:age "41"^^xsd:integer ; ex:score 2.5 .
ex:dave ex:name "Dave the very long name" ; ex:nick _:n1 .
_:n1 ex:label "nick" .
"#;

pub const TRIG: &str = r#"
@prefix ex: <http://ex/> .
ex:g1 { ex:alice ex:likes ex:tea . }
ex:g2 { ex:bob ex:likes ex:coffee . }
"#;

/// In-memory connection with sparqlite registered.
pub fn conn() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    sq_api::register(&c).unwrap();
    c
}

/// In-memory connection with the schema and [`DATA`] loaded.
pub fn loaded() -> Connection {
    let c = conn();
    let n: i64 = c.query_row("SELECT sparql_load(?1)", [DATA], |r| r.get(0)).unwrap();
    assert_eq!(n, 13);
    c
}

pub fn one<T: rusqlite::types::FromSql>(c: &Connection, sql: &str) -> T {
    c.query_row(sql, [], |r| r.get(0)).unwrap_or_else(|e| panic!("{sql}: {e}"))
}

pub fn one_p<T: rusqlite::types::FromSql, P: rusqlite::Params>(c: &Connection, sql: &str, p: P) -> T {
    c.query_row(sql, p, |r| r.get(0)).unwrap_or_else(|e| panic!("{sql}: {e}"))
}

pub fn err<P: rusqlite::Params>(c: &Connection, sql: &str, p: P) -> String {
    match c.query_row(sql, p, |r| r.get::<_, rusqlite::types::Value>(0)) {
        Ok(v) => panic!("{sql}: expected an error, got {v:?}"),
        Err(e) => e.to_string(),
    }
}

/// A `sparql_status` value.
pub fn status(c: &Connection, key: &str) -> rusqlite::types::Value {
    c.query_row("SELECT value FROM sparql_status WHERE key = ?1", [key], |r| r.get(0))
        .optional()
        .unwrap()
        .unwrap_or_else(|| panic!("no status key {key}"))
}

pub fn status_i64(c: &Connection, key: &str) -> i64 {
    match status(c, key) {
        rusqlite::types::Value::Integer(v) => v,
        other => panic!("{key} = {other:?}"),
    }
}

/// A temporary file path removed (with its -wal / -shm / -journal siblings) on drop.
pub struct TempPath(pub PathBuf);

impl TempPath {
    pub fn new(name: &str) -> TempPath {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join("sq-api-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{name}-{}-{n}.db", std::process::id()));
        let t = TempPath(p);
        t.remove();
        t
    }

    pub fn sibling(&self, suffix: &str) -> PathBuf {
        let mut s = self.0.clone().into_os_string();
        s.push(suffix);
        PathBuf::from(s)
    }

    fn remove(&self) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(self.sibling(suffix));
        }
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        self.remove();
    }
}
