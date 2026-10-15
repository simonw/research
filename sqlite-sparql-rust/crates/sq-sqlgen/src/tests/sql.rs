use std::rc::Rc;

use crate::expr::is_simple_sql;
use crate::sql::{Block, Col, Item, Params, and_all, or_all};
use crate::translate::{Env, GraphCtx, bind, compat, join};

#[test]
fn parameters_are_deduplicated_and_renumbered_by_first_use() {
    let mut p = Params::default();
    assert_eq!(p.get(42), "?1");
    assert_eq!(p.get(7), "?2");
    assert_eq!(p.get(42), "?1");
    assert_eq!(p.get(9), "?3");
    // ?1 was allocated for a fragment that was dropped; ?3 is used before ?2.
    let (sql, values) = p.finish("SELECT ?3 WHERE x = ?2 AND y = ?3");
    assert_eq!(sql, "SELECT ?1 WHERE x = ?2 AND y = ?1");
    assert_eq!(values, [9, 7]);
    let (sql, values) = Params::default().finish("SELECT 1");
    assert_eq!((sql.as_str(), values.len()), ("SELECT 1", 0));
}

#[test]
fn compatibility_conditions() {
    let a = Col::new("t1.s");
    let b = Col::new("t2.s");
    let na = Col::nullable("t3.o");
    let nb = Col::nullable("t4.o");
    assert_eq!(compat(&a, &b), (Some("t1.s = t2.s".into()), a.clone()));
    assert_eq!(compat(&a, &nb), (Some("(t4.o IS NULL OR t4.o = t1.s)".into()), a.clone()));
    assert_eq!(compat(&na, &b), (Some("(t3.o IS NULL OR t3.o = t2.s)".into()), b.clone()));
    assert_eq!(
        compat(&na, &nb),
        (Some("(t3.o IS NULL OR t4.o IS NULL OR t3.o = t4.o)".into()), Col::nullable("COALESCE(t3.o, t4.o)"))
    );
    assert_eq!(compat(&a, &a), (None, a.clone()));
}

#[test]
fn binding_under_an_exists_substitution_constrains_the_first_occurrence() {
    let mut outer = std::collections::BTreeMap::new();
    outer.insert(3, Col::nullable("t9.o"));
    let env = Env { graph: GraphCtx::Default, outer: Rc::new(outer) };
    let mut b = Block::unit();
    bind(&mut b, 3, Col::new("t1.s"), &env);
    bind(&mut b, 3, Col::new("t2.s"), &env);
    assert_eq!(b.conds, ["(t9.o IS NULL OR t9.o = t1.s)", "t1.s = t2.s"]);
    assert_eq!(b.cols[&3], Col::new("t1.s"));
}

#[test]
fn joins_concatenate_and_join_shared_slots() {
    let mut a = Block::unit();
    a.items.push(Item::Quad("t1".into()));
    a.cols.insert(0, Col::new("t1.s"));
    let mut b = Block::unit();
    b.items.push(Item::Quad("t2".into()));
    b.items.push(Item::Left { item: Box::new(Item::Quad("t3".into())), on: "t3.s = t2.o".into() });
    b.conds.push("t2.p = ?1".into());
    b.cols.insert(0, Col::new("t2.s"));
    b.cols.insert(1, Col::nullable("t3.o"));
    let j = join(a, b);
    assert_eq!(
        j.render(&[0, 1, 2], false),
        "SELECT t1.s AS v0, t3.o AS v1, NULL AS v2 FROM main.rdf_quad AS t1, main.rdf_quad AS t2 \
         LEFT JOIN main.rdf_quad AS t3 ON t3.s = t2.o WHERE t2.p = ?1 AND t1.s = t2.s"
    );
    assert!(join(j, Block::empty()).empty);
}

#[test]
fn rendering_corner_cases() {
    assert_eq!(Block::unit().render(&[], false), "SELECT NULL AS u");
    assert_eq!(Block::empty().render(&[4], true), "SELECT DISTINCT NULL AS v4 WHERE 0");
    let mut d = Block::unit();
    d.items.push(Item::Derived { sql: "VALUES (?1)".into(), alias: "d1".into() });
    assert!(d.is_single_item());
    assert_eq!(d.from_sql(), " FROM (VALUES (?1)) AS d1");
    assert_eq!(and_all(&[]), "1");
    assert_eq!(and_all(&["a".into(), "b".into()]), "a AND b");
    assert_eq!(or_all(&[]), "0");
    assert_eq!(or_all(&["a".into()]), "a");
    assert_eq!(or_all(&["a".into(), "b".into()]), "(a OR b)");
}

#[test]
fn simple_sql_is_a_column_or_a_parameter() {
    for s in ["t1.s", "d12.v3", "d4.column2", "?7"] {
        assert!(is_simple_sql(s), "{s}");
    }
    for s in ["?", "?a", "COALESCE(t1.s, t2.s)", "sparql_eval('+', t1.o, ?1)", "NULL", "t1.s + 1", ".s", "t1."] {
        assert!(!is_simple_sql(s), "{s}");
    }
}
