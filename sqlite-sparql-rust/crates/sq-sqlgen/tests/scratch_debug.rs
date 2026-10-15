mod common;
use common::{DataFile, Engine};
#[test]
fn scratch() {
    let engine = Engine::new(&[DataFile::turtle("@prefix : <http://example.org/> . :A0 :P :A1 . :A1 :P :A2 .")]).unwrap();
    let q = std::env::var("Q").unwrap();
    let query = sq_plan::parse_query(&q, None).unwrap();
    let lowered = engine.lower(&query).unwrap();
    let mut t = sq_core::ir::LoweredQuery::clone(&lowered);
    let _ = &mut t;
    match sq_sqlgen::translate(&lowered) {
        Ok(sql) => { println!("SQL: {}", sql.sql); println!("{:?}", engine.execute(&lowered, &sql).map(|_| "ok")); }
        Err(e) => println!("ERR {e}"),
    }
}
