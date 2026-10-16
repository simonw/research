//! The SQLite-limits model (`limits`): calibration against SQLite 3.45.1 and the per-level
//! limits.
use crate::limits::{check, estimated_stack};

/// (entries SQLite 3.45.1 needs, statement): `need` = 100 − the number of `(…)` that still fit
/// around the innermost `1` (measured with the system libsqlite3; see the module docs). The
/// model must never estimate less.
const CALIBRATION: &[(u32, &str)] = &[
    (7, "SELECT 1"),
    (7, "SELECT 1 AS a, 1 AS b"),
    (12, "SELECT f(1, 1) AS b"),
    (8, "SELECT 1 FROM q WHERE 1"),
    (10, "SELECT 1 FROM q WHERE q.g = 0 AND q.s = 1 AND 1"),
    (12, "SELECT 1 FROM q WHERE q.g = 0 AND q.s = 1"),
    (16, "SELECT 1 FROM q WHERE q.g = 0 AND EXISTS (SELECT 1)"),
    (17, "SELECT 1 FROM q WHERE q.g = 0 AND NOT EXISTS (SELECT 1)"),
    (14, "SELECT 1 FROM q AS a LEFT JOIN q AS b ON b.g = 0 AND 1"),
    (17, "SELECT 1 FROM q AS a LEFT JOIN (SELECT 1 AS v) AS d ON a.s = d.v AND a.g = 1"),
    (15, "SELECT 1 FROM q AS a, q AS c LEFT JOIN (SELECT 1 AS v) AS d ON a.s = d.v AND 1"),
    (10, "SELECT 1 FROM q AS a LEFT JOIN (SELECT 1 AS v) AS d ON a.s = d.v WHERE a.g = 0 AND 1"),
    (13, "SELECT 1 FROM q AS a, (SELECT 1 AS v) AS d"),
    (13, "SELECT 1 FROM q AS a LEFT JOIN (SELECT 1 AS v) AS d ON 1"),
    (12, "SELECT 1 FROM q GROUP BY q.s, 1"),
    (14, "SELECT 1 FROM q ORDER BY q.s DESC, 1"),
    (14, "SELECT 1 FROM q LIMIT 1 OFFSET 1"),
    (9, "SELECT 1 UNION ALL SELECT 1"),
    (12, "SELECT 1 UNION ALL SELECT 1 FROM q WHERE q.g = 0 AND 1"),
    (13, "WITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT n FROM r WHERE 0) SELECT n FROM r"),
    (
        20,
        "WITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT k.o FROM r, q AS k WHERE k.s = r.n AND k.g = 1) SELECT n \
         FROM r",
    ),
    (
        13,
        "WITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT k.o FROM r, q AS k WHERE k.s = r.n) SELECT n FROM r WHERE n \
         = 1",
    ),
    (18, "SELECT row_number() OVER (ORDER BY sparql_sortkey(q.s), 1) FROM q"),
    (14, "SELECT * FROM (VALUES (1, 2), (3, 1)) AS d"),
    (13, "SELECT CASE WHEN 1 THEN 1 WHEN 2 THEN 1 END"),
    (11, "SELECT CASE 1 WHEN 1 THEN 23 WHEN 1 THEN 7 END"),
    (16, "SELECT CASE WHEN 1 = 6 AND 2 BETWEEN -5 AND 1 THEN 1 END"),
    (14, "SELECT CASE WHEN 1 = 6 THEN 2 >= 1 END"),
    (12, "SELECT 1 IN (2, 3, 1)"),
    (11, "SELECT 1 FROM q WHERE q.g = 0 AND NOT 1"),
    (13, "SELECT 1 FROM q WHERE (q.s IS NULL OR q.s = 1)"),
    (9, "SELECT CAST(1 AS REAL)"),
    (9, "SELECT (1 << 4) | 1"),
    (12, "SELECT COALESCE(1, 2, 1)"),
    (
        23,
        "SELECT 1 FROM q WHERE q.g = 0 AND (q.s IS NULL OR CASE WHEN q.s = 1 THEN sparql_num(q.s) > (CAST(1 AS \
         REAL) / 1) END)",
    ),
    (21, "SELECT 1 FROM q AS z WHERE z.g = 0 AND EXISTS (SELECT 1 FROM q AS t0 WHERE t0.g = 0 AND 1 = 1)"),
    (
        30,
        "SELECT 1 FROM q AS z WHERE z.g = 0 AND EXISTS (SELECT 1 FROM q AS t0 WHERE t0.g = 0 AND EXISTS (SELECT 1 \
         FROM q AS t1 WHERE t1.g = 0 AND 1 = 1))",
    ),
    (
        39,
        "SELECT 1 FROM q AS z WHERE z.g = 0 AND EXISTS (SELECT 1 FROM q AS t0 WHERE t0.g = 0 AND EXISTS (SELECT 1 \
         FROM q AS t1 WHERE t1.g = 0 AND EXISTS (SELECT 1 FROM q AS t2 WHERE t2.g = 0 AND 1 = 1)))",
    ),
    (21, "SELECT 1 FROM q AS z WHERE z.g = 0 AND (NOT EXISTS (SELECT 1 FROM q AS t0 WHERE t0.g = 0 AND 1))"),
    (
        32,
        "SELECT 1 FROM q AS z WHERE z.g = 0 AND (NOT EXISTS (SELECT 1 FROM q AS t0 WHERE t0.g = 0 AND (NOT EXISTS \
         (SELECT 1 FROM q AS t1 WHERE t1.g = 0 AND 1))))",
    ),
    (
        43,
        "SELECT 1 FROM q AS z WHERE z.g = 0 AND (NOT EXISTS (SELECT 1 FROM q AS t0 WHERE t0.g = 0 AND (NOT EXISTS \
         (SELECT 1 FROM q AS t1 WHERE t1.g = 0 AND (NOT EXISTS (SELECT 1 FROM q AS t2 WHERE t2.g = 0 AND 1))))))",
    ),
    (13, "SELECT d0.v0 AS v0 FROM main.q AS t0 LEFT JOIN (SELECT 1 AS v0) AS d0 ON t0.s = d0.v0"),
    (
        19,
        "SELECT d0.v0 AS v0 FROM main.q AS t0 LEFT JOIN (SELECT d1.v0 AS v0 FROM main.q AS t1 LEFT JOIN (SELECT 1 \
         AS v0) AS d1 ON t1.s = d1.v0) AS d0 ON t0.s = d0.v0",
    ),
    (
        25,
        "SELECT d0.v0 AS v0 FROM main.q AS t0 LEFT JOIN (SELECT d1.v0 AS v0 FROM main.q AS t1 LEFT JOIN (SELECT \
         d2.v0 AS v0 FROM main.q AS t2 LEFT JOIN (SELECT 1 AS v0) AS d2 ON t2.s = d2.v0) AS d1 ON t1.s = d1.v0) AS \
         d0 ON t0.s = d0.v0",
    ),
    (15, "SELECT 1 AS v0 FROM q UNION ALL SELECT d0.v0 FROM (SELECT 1 AS v0) AS d0"),
    (
        23,
        "SELECT 1 AS v0 FROM q UNION ALL SELECT d0.v0 FROM (SELECT 1 AS v0 FROM q UNION ALL SELECT d1.v0 FROM \
         (SELECT 1 AS v0) AS d1) AS d0",
    ),
    (
        31,
        "SELECT 1 AS v0 FROM q UNION ALL SELECT d0.v0 FROM (SELECT 1 AS v0 FROM q UNION ALL SELECT d1.v0 FROM \
         (SELECT 1 AS v0 FROM q UNION ALL SELECT d2.v0 FROM (SELECT 1 AS v0) AS d2) AS d1) AS d0",
    ),
    (15, "SELECT 1 FROM q AS z WHERE z.g = 0 AND sparql_eval('+', z.s, 1) = 23"),
    (
        22,
        "SELECT 1 FROM q AS z WHERE z.g = 0 AND sparql_eval('+', z.s, z.g = 0 AND sparql_eval('+', z.s, 1) = 23) = \
         23",
    ),
    (
        29,
        "SELECT 1 FROM q AS z WHERE z.g = 0 AND sparql_eval('+', z.s, z.g = 0 AND sparql_eval('+', z.s, z.g = 0 AND \
         sparql_eval('+', z.s, 1) = 23) = 23) = 23",
    ),
    (14, "SELECT 1 FROM q AS z WHERE CASE WHEN z.s & 15 = 6 THEN z.s > 1 WHEN z.s & 15 = 8 THEN 1 END"),
    (
        20,
        "SELECT 1 FROM q AS z WHERE CASE WHEN z.s & 15 = 6 THEN z.s > 1 WHEN z.s & 15 = 8 THEN CASE WHEN z.s & 15 = \
         6 THEN z.s > 1 WHEN z.s & 15 = 8 THEN 1 END END",
    ),
    (
        26,
        "SELECT 1 FROM q AS z WHERE CASE WHEN z.s & 15 = 6 THEN z.s > 1 WHEN z.s & 15 = 8 THEN CASE WHEN z.s & 15 = \
         6 THEN z.s > 1 WHEN z.s & 15 = 8 THEN CASE WHEN z.s & 15 = 6 THEN z.s > 1 WHEN z.s & 15 = 8 THEN 1 END END \
         END",
    ),
    (
        19,
        "WITH RECURSIVE r0(n) AS (SELECT o FROM (SELECT 1 AS o) UNION SELECT k.o FROM r0, q AS k WHERE k.s = r0.n) \
         SELECT n AS o FROM r0",
    ),
    (
        31,
        "WITH RECURSIVE r0(n) AS (SELECT o FROM (WITH RECURSIVE r1(n) AS (SELECT o FROM (SELECT 1 AS o) UNION \
         SELECT k.o FROM r1, q AS k WHERE k.s = r1.n) SELECT n AS o FROM r1) UNION SELECT k.o FROM r0, q AS k WHERE \
         k.s = r0.n) SELECT n AS o FROM r0",
    ),
    (
        43,
        "WITH RECURSIVE r0(n) AS (SELECT o FROM (WITH RECURSIVE r1(n) AS (SELECT o FROM (WITH RECURSIVE r2(n) AS \
         (SELECT o FROM (SELECT 1 AS o) UNION SELECT k.o FROM r2, q AS k WHERE k.s = r2.n) SELECT n AS o FROM r2) \
         UNION SELECT k.o FROM r1, q AS k WHERE k.s = r1.n) SELECT n AS o FROM r1) UNION SELECT k.o FROM r0, q AS k \
         WHERE k.s = r0.n) SELECT n AS o FROM r0",
    ),
    (
        19,
        "SELECT v0 FROM (SELECT d0.v0 AS v0, row_number() OVER (ORDER BY sparql_sortkey(d0.v0)) AS rn FROM (SELECT \
         1 AS v0) AS d0) GROUP BY v0 ORDER BY min(rn)",
    ),
    (
        31,
        "SELECT v0 FROM (SELECT d0.v0 AS v0, row_number() OVER (ORDER BY sparql_sortkey(d0.v0)) AS rn FROM (SELECT \
         v0 FROM (SELECT d1.v0 AS v0, row_number() OVER (ORDER BY sparql_sortkey(d1.v0)) AS rn FROM (SELECT 1 AS \
         v0) AS d1) GROUP BY v0 ORDER BY min(rn)) AS d0) GROUP BY v0 ORDER BY min(rn)",
    ),
    (
        43,
        "SELECT v0 FROM (SELECT d0.v0 AS v0, row_number() OVER (ORDER BY sparql_sortkey(d0.v0)) AS rn FROM (SELECT \
         v0 FROM (SELECT d1.v0 AS v0, row_number() OVER (ORDER BY sparql_sortkey(d1.v0)) AS rn FROM (SELECT v0 FROM \
         (SELECT d2.v0 AS v0, row_number() OVER (ORDER BY sparql_sortkey(d2.v0)) AS rn FROM (SELECT 1 AS v0) AS d2) \
         GROUP BY v0 ORDER BY min(rn)) AS d1) GROUP BY v0 ORDER BY min(rn)) AS d0) GROUP BY v0 ORDER BY min(rn)",
    ),
    (18, "SELECT 1 FROM q AS z WHERE z.g = 0 AND (z.s IS NULL OR CASE WHEN z.s = 1 THEN 1 END)"),
    (
        28,
        "SELECT 1 FROM q AS z WHERE z.g = 0 AND (z.s IS NULL OR CASE WHEN z.s = 1 THEN z.g = 0 AND (z.s IS NULL OR \
         CASE WHEN z.s = 1 THEN 1 END) END)",
    ),
    (
        38,
        "SELECT 1 FROM q AS z WHERE z.g = 0 AND (z.s IS NULL OR CASE WHEN z.s = 1 THEN z.g = 0 AND (z.s IS NULL OR \
         CASE WHEN z.s = 1 THEN z.g = 0 AND (z.s IS NULL OR CASE WHEN z.s = 1 THEN 1 END) END) END)",
    ),
];

#[test]
fn the_parser_stack_model_bounds_every_measurement() {
    let mut failures = Vec::new();
    for (need, sql) in CALIBRATION {
        let estimate = estimated_stack(sql);
        if estimate < *need {
            failures.push(format!("estimated {estimate} < measured {need}: {sql}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn deep_nesting_is_rejected() {
    let mut cond = "1".to_owned();
    for i in 0..12 {
        cond = format!("EXISTS (SELECT 1 FROM main.rdf_quad AS t{i} WHERE t{i}.g = 0 AND {cond})");
    }
    assert!(check(&format!("SELECT 1 WHERE {cond}")).is_err(), "12 nested EXISTS");
    let shallow = "SELECT 1 FROM main.rdf_quad AS t1 WHERE t1.g = 0 AND EXISTS (SELECT 1 FROM main.rdf_quad AS t2 \
                   WHERE t2.g = 0 AND t2.s = t1.s)";
    assert!(check(shallow).is_ok());
}

#[test]
fn per_level_limits() {
    let args = |n: usize| format!("SELECT sparql_eval('CONCAT'{})", ", ?1".repeat(n));
    assert!(check(&args(126)).is_ok(), "127 arguments");
    assert!(check(&args(127)).is_err(), "128 arguments");
    let terms = |n: usize| vec!["SELECT 1"; n].join(" UNION ALL ");
    assert!(check(&terms(500)).is_ok());
    assert!(check(&terms(501)).is_err());
    let columns = |n: usize| format!("SELECT {} FROM main.rdf_quad", vec!["g"; n].join(", "));
    assert!(check(&columns(2000)).is_ok());
    assert!(check(&columns(2001)).is_err());
    // Commas elsewhere are neither columns nor arguments.
    let items = format!("SELECT 1 FROM {}", vec!["main.rdf_quad"; 3000].join(", "));
    assert!(check(&items).is_ok());
    let values = format!("SELECT * FROM (VALUES {}) AS d", vec!["(?1, ?2)"; 3000].join(", "));
    assert!(check(&values).is_ok());
    assert!(check("SELECT sparql_eval('a''(b', ?1)").is_ok(), "string literals are opaque");
}

#[test]
fn expression_depth() {
    let flat = |n: usize| format!("SELECT 1 WHERE {}", vec!["?1 = 1"; n].join(" AND "));
    assert!(check(&flat(900)).is_ok());
    assert!(check(&flat(1000)).is_err(), "a left-deep AND chain of 1000");
    // Every term of a compound has its own WHERE.
    let terms = vec![format!("SELECT 1 WHERE {}", vec!["?1 = 1"; 600].join(" AND ")); 400];
    assert!(check(&terms.join(" UNION ALL ")).is_ok());
    let conds: Vec<String> = vec!["?1 = 1".to_owned(); 5000];
    assert!(check(&format!("SELECT 1 WHERE {}", crate::sql::and_all(&conds))).is_ok(), "balanced runs");
}
