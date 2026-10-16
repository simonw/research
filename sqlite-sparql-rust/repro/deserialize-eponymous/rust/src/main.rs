use rusqlite::{ffi, Connection};
use std::os::raw::c_uint;

fn image() -> (*mut u8, i64) {
    let o = Connection::open_in_memory().unwrap();
    o.execute_batch("create table t(a primary key, b) without rowid").unwrap();
    let mut size = 0i64;
    let p = unsafe { ffi::sqlite3_serialize(o.handle(), c"main".as_ptr(), &mut size, 0) };
    assert!(!p.is_null());
    (p.cast(), size)
}

fn run(c: &Connection, sql: &str) -> Vec<String> {
    let mut st = c.prepare(sql).unwrap(); // fresh prepare each time (no statement cache)
    let n = st.column_count();
    st.query_map([], |r| Ok((0..n).map(|i| format!("{:?}", r.get_ref(i).unwrap())).collect::<Vec<_>>().join(",")))
        .unwrap().map(|r| r.unwrap()).collect()
}

fn main() {
    let sql = std::env::args().nth(1).unwrap_or("SELECT * FROM pragma_index_info('sqlite_autoindex_t_1')".into());
    println!("sqlite {}", rusqlite::version());
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("create table t(a primary key, b) without rowid").unwrap();
    println!("before {:?}", run(&c, &sql));
    let (p, n) = image();
    let flags = (ffi::SQLITE_DESERIALIZE_FREEONCLOSE | ffi::SQLITE_DESERIALIZE_RESIZEABLE) as c_uint;
    let rc = unsafe { ffi::sqlite3_deserialize(c.handle(), c"main".as_ptr(), p, n, n, flags) };
    assert_eq!(rc, 0);
    println!("after  {:?}", run(&c, &sql));
}
