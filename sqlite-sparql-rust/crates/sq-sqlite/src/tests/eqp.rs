//! §5.2: `EXPLAIN QUERY PLAN` of every reachable `StmtKey` uses the expected covering index,
//! constrains exactly the expected `(col=? …)` prefix / range, and never sorts.
use sq_core::store::IndexOrder;

use super::Db;
use crate::template::{StmtKey, StmtKind, UpperOp, COL_NAMES};

/// `NOT INDEXED` alone would not pin the GSPO primary key (SQLite 3.45.1 picks rdf_gpso for
/// `g=? AND s=? AND p=? AND o>=? ORDER BY g,s,p,o`); the templates name its autoindex.
#[test]
fn not_indexed_does_not_pin_the_primary_key() {
    let db = Db::mem();
    let plan = db
        .store
        .explain_query_plan("SELECT o FROM main.rdf_quad NOT INDEXED WHERE g=?1 AND s=?2 AND p=?3 AND o>=?4 ORDER BY g,s,p,o")
        .unwrap();
    assert!(plan.contains("rdf_gpso"), "{plan}");
}

fn index_name(order: IndexOrder) -> String {
    match order {
        IndexOrder::Gspo => "PRIMARY KEY".to_string(),
        o => format!("COVERING INDEX {}", o.sql_from().rsplit(' ').next().unwrap()),
    }
}

/// The constraint list SQLite prints for a key, e.g. `g=? AND p=? AND s>? AND s<?`.
fn expected_constraints(key: &StmtKey) -> Vec<String> {
    let cols = key.order.cols();
    let k = key.prefix_len as usize;
    let mut terms: Vec<String> = (0..k).map(|i| format!("{}=?", COL_NAMES[cols[i] as usize])).collect();
    if k < 4 {
        let c = COL_NAMES[cols[k] as usize];
        if key.seek_cols >= 2 {
            let m = key.seek_cols as usize;
            let lhs: Vec<&str> = cols[k..k + m].iter().map(|c| COL_NAMES[*c as usize]).collect();
            terms.push(format!("({})>({})", lhs.join(","), vec!["?"; m].join(",")));
        } else {
            terms.push(format!("{c}>?"));
        }
        if key.upper != UpperOp::None {
            terms.push(format!("{c}<?"));
        }
    }
    terms
}

/// Parses `... USING <index> (<constraints>)` from a plan line.
fn parse_line(line: &str) -> (String, Vec<String>) {
    let using = line.split(" USING ").nth(1).unwrap_or("");
    let (index, cons) = match using.find(" (") {
        Some(i) => (&using[..i], &using[i + 2..using.len() - 1]),
        None => (using, ""),
    };
    let cons = if cons.is_empty() { Vec::new() } else { cons.split(" AND ").map(str::to_string).collect() };
    (index.to_string(), cons)
}

#[test]
fn every_statement_key_uses_its_index_without_sorting() {
    let db = Db::mem();
    let keys = StmtKey::all_reachable();
    assert!(keys.len() > 500, "{} keys", keys.len());
    for key in keys {
        let sql = key.sql();
        let plan = db.store.explain_query_plan(&sql).unwrap();
        assert!(!plan.contains("TEMP B-TREE"), "{key:?}: sorter in plan\n{sql}\n{plan}");
        let line = plan
            .lines()
            .find(|l| l.contains("rdf_quad") && (l.contains("SEARCH") || l.contains("SCAN")))
            .unwrap_or_else(|| panic!("{key:?}: no rdf_quad access in\n{plan}"));
        let (index, got) = parse_line(line);
        assert_eq!(index, index_name(key.order), "{key:?}\n{sql}\n{plan}");
        assert_eq!(got, expected_constraints(&key), "{key:?}\n{sql}\n{plan}");
    }
}

#[test]
fn templates_render_as_specified() {
    use crate::template::{LowerOp, StmtKey};
    let k = StmtKey { order: IndexOrder::Gpso, prefix_len: 2, lower: LowerOp::Ge, upper: UpperOp::None, seek_cols: 0, kind: StmtKind::Scan };
    assert_eq!(k.sql(), "SELECT s,o FROM main.rdf_quad INDEXED BY rdf_gpso WHERE g=?1 AND p=?2 AND s>=?3 ORDER BY g,p,s,o");
    assert_eq!(
        k.seek_variant(2).sql(),
        "SELECT s,o FROM main.rdf_quad INDEXED BY rdf_gpso WHERE g=?1 AND p=?2 AND (s,o)>=(?3,?4) ORDER BY g,p,s,o"
    );
    let k = StmtKey { order: IndexOrder::Psog, prefix_len: 1, lower: LowerOp::Gt, upper: UpperOp::Lt, seek_cols: 0, kind: StmtKind::Scan };
    assert_eq!(k.sql(), "SELECT s,o,g FROM main.rdf_quad INDEXED BY rdf_psog WHERE p=?1 AND g<>0 AND s>?2 AND s<?3 ORDER BY p,s,o,g");
    let k = StmtKey { order: IndexOrder::Gpos, prefix_len: 3, lower: LowerOp::Ge, upper: UpperOp::None, seek_cols: 0, kind: StmtKind::CountCapped };
    assert_eq!(
        k.sql(),
        "SELECT count(*) FROM (SELECT 1 FROM main.rdf_quad INDEXED BY rdf_gpos WHERE g=?1 AND p=?2 AND o=?3 AND s>=?4 LIMIT ?5)"
    );
    let k = StmtKey { order: IndexOrder::Gpox, prefix_len: 2, lower: LowerOp::Ge, upper: UpperOp::Le, seek_cols: 0, kind: StmtKind::Count };
    assert_eq!(
        k.sql(),
        "SELECT count(*) FROM main.rdf_quad INDEXED BY rdf_gpox WHERE g=?1 AND p=?2 AND (o & 15) IN (5,8) AND o>=?3 AND o<=?4"
    );
    let k = StmtKey { order: IndexOrder::Gspo, prefix_len: 4, lower: LowerOp::Ge, upper: UpperOp::None, seek_cols: 0, kind: StmtKind::Scan };
    assert_eq!(k.sql(), "SELECT 1 FROM main.rdf_quad INDEXED BY sqlite_autoindex_rdf_quad_1 WHERE g=?1 AND s=?2 AND p=?3 AND o=?4");
}

#[test]
fn fixed_statements_use_indexes() {
    use crate::template::Fixed;
    let db = Db::mem();
    for (f, want) in [
        (Fixed::TermByHash, "rdf_term_h"),
        (Fixed::TermById, "INTEGER PRIMARY KEY"),
        (Fixed::TermScanFrom, "INTEGER PRIMARY KEY"),
        (Fixed::NextGraph, "(g>?)"),
        (Fixed::GraphHasQuads, "(g=?)"),
        (Fixed::ClearGraph, "(g=?)"),
    ] {
        let plan = db.store.explain_query_plan(f.sql()).unwrap();
        assert!(plan.contains(want) && !plan.contains("TEMP B-TREE"), "{f:?}: {plan}");
    }
}
