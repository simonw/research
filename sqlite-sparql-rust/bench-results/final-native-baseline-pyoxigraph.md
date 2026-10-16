## sparqlite benchmark

Run 2026-10-16T01:50:16+00:00 on Intel(R) Xeon(R) Processor @ 2.80GHz (4 CPUs, 16 GiB); SQLite 3.45.1, Python 3.13.16, pyoxigraph 0.5.11, rdflib 7.6.0, sparqlite 0.1.0 (SQLite 3.45.1); git f2ba0a122e95; harness `91e26652b40e`.

Settings: modes count, full; median of 5 after 1 warm-up; timeout 120 s; cell budget 120 s; json mode off (`sparql_query` and the baseline's `sparql_query_sql` not measured); full columns `terms`; reference count `aggregate`; native config default; SQLite load `auto`; pyoxigraph/rdflib in worker processes; rdflib up to 100,000 triples. Command: `py/bench.py --social $SCRATCH/data/social-200k.nt --bsbm $SCRATCH/data/bsbm-1m.nt --engines native,baseline,pyoxigraph --modes count,full --ext $SCRATCH/targets/ext/release/libsparqlite --work-dir $SCRATCH/work/benchFinal --timeout 120 --out-md $SCRATCH/work/benchFinal/results.md --out-json $SCRATCH/work/benchFinal/results.json`.

Speed-ups in parentheses are the other engine's time divided by this engine's (>1 = this engine is faster): `×b` vs sparqlite-baseline, `×o` vs pyoxigraph. † = fewer repetitions than requested (cell budget 120 s).

### Datasets and load times

| Dataset | Triples | File MB | SHA-256 | native | baseline | pyoxigraph |
|---|---:|---:|---|---:|---:|---:|
| social | 1,600,000 | 175.0 | `b0c8f3a5adeb` | 10.46 s, 129 MB | (same DB as native) | 8.77 s |
| bsbm | 1,004,103 | 146.9 | `6586dcc57a51` | 7.08 s, 90 MB | (same DB as native) | 3.99 s |

- sparqlite-native on social: load 9.06 s (file), ANALYZE 0.76 s, sparql_analyze 0.64 s
- sparqlite-native on bsbm: load 6.12 s (file), ANALYZE 0.54 s, sparql_analyze 0.42 s

### count mode (solutions counted inside each engine, no result term materialised)

Median ms of 5 runs after 1 warm-up. native: `sparql_count(q)`; baseline: `sparql_count_sql(q)`; pyoxigraph: `Store.query(SELECT (COUNT(*) AS ?n) { q })`.

| Query | Data | Rows | native | baseline | pyoxigraph |
|---|---|---:|---:|---:|---:|
| Q1 star-4 (class + age + city + name) | social | 20,000 | 32.9 (1.40×b, 2.71×o) | 46.0 (1.93×o) | 88.9 (0.52×b) |
| Q2 BSBM product star-6 | bsbm | 1,606 | 2.92 (1.89×b, 4.91×o) | 5.52 (2.60×o) | 14.3 (0.39×b) |
| Q3 chain-2 COUNT | social | 1 | 49.5 (8.91×b, 35.9×o) | 441 (4.03×o) | 1,779 (0.25×b) |
| Q4 selective chain-3 from a constant | social | 27 | 0.145 (0.45×b, 0.96×o) | 0.064 (2.14×o) | 0.138 (0.47×b) |
| Q5 OPTIONAL with a type check inside | social | 20,634 | 26.1 (3.17×b) | 82.6 | timeout |
| Q6 FILTER(?a > 90) | social | 1,393 | 0.149 (151×b, 678×o) | 22.5 (4.50×o) | 101 (0.22×b) |
| Q7 REGEX on names | social | 11,111 | 72.9 (1.88×b, 1.64×o) | 137 (0.88×o) | 120 (1.14×b) |
| Q8 GROUP BY + COUNT + ORDER BY DESC LIMIT 10 | social | 10 | 17.2 (1.34×b, 6.18×o) | 23.1 (4.60×o) | 106 (0.22×b) |
| Q9 ORDER BY DESC(?age) ?x LIMIT 10 | social | 10 | 9.78 (19.3×b, 153×o) | 189 (7.89×o) | 1,492 (0.13×b) |
| Q10 DISTINCT over a join | social | 9,999 | 158 (6.10×b, 14.7×o) | 964 (2.41×o) | 2,325 (0.41×b) |
| Q11 UNION | social | 127,794 | 37.0 (1.16×b, 4.16×o) | 42.9 (3.59×o) | 154 (0.28×b) |
| Q12 MINUS | social | 14,877 | 24.8 (2.24×b, 5.76×o) | 55.6 (2.57×o) | 143 (0.39×b) |
| Q13 :p1 :knows+ ?y COUNT | social | 1 | 175 (2.66×b, 1.80×o) | 466 (0.68×o) | 315 (1.48×b) |
| Q14 ?x rdf:type/rdfs:subClassOf* bsbm:Product | bsbm | 7,800 | 0.733 (2.05×b, 156×o) | 1.50 (76.0×o) | 114 (0.013×b) |
| Q15 point lookup :p123 ?p ?o | social | 8 | 0.089 (1.03×b, 1.53×o) | 0.092 (1.49×o) | 0.137 (0.67×b) |
| Q16 COUNT(*) {?s ?p ?o} | social | 1 | 2.74 (22.2×b, 142×o) | 61.0 (6.37×o) | 388 (0.16×b) |
| Q17 BSBM offers: price range + date filter + ORDER BY price LIMIT 10 | bsbm | 10 | 24.8 (3.20×b, 2.58×o) | 79.2 (0.81×o) | 63.8 (1.24×b) |

Geometric-mean speed-up (>1 = faster): pyoxigraph vs baseline: 0.33× over 16 queries; baseline vs pyoxigraph: 3.04× over 16 queries; native vs pyoxigraph: 10.6× over 16 queries; native vs baseline: 3.47× over 17 queries.

### full mode (one Python string per projected result term, on every engine)

Median ms of 5 runs after 1 warm-up. native: `SELECT t0..t{n-1} FROM sparql_rows(q)`; baseline: `SELECT t0..t{n-1} FROM sparql_rows(q, '{"engine":"sql"}')`; pyoxigraph: `str()` of every term.

| Query | Data | Rows | native | baseline | pyoxigraph |
|---|---|---:|---:|---:|---:|
| Q1 star-4 (class + age + city + name) | social | 20,000 | 82.3 (1.25×b, 2.17×o) | 103 (1.73×o) | 178 (0.58×b) |
| Q2 BSBM product star-6 | bsbm | 1,606 | 9.94 (1.19×b, 2.41×o) | 11.8 (2.03×o) | 24.0 (0.49×b) |
| Q3 chain-2 COUNT | social | 1 | 50.0 (9.22×b, 37.1×o) | 461 (4.02×o) | 1,855 (0.25×b) |
| Q4 selective chain-3 from a constant | social | 27 | 0.175 (0.76×b, 0.93×o) | 0.132 (1.23×o) | 0.163 (0.81×b) |
| Q5 OPTIONAL with a type check inside | social | 20,634 | 47.7 (2.32×b) | 111 | timeout |
| Q6 FILTER(?a > 90) | social | 1,393 | 2.10 (12.0×b, 50.7×o) | 25.1 (4.24×o) | 107 (0.24×b) |
| Q7 REGEX on names | social | 11,111 | 102 (1.64×b, 1.33×o) | 168 (0.81×o) | 136 (1.23×b) |
| Q8 GROUP BY + COUNT + ORDER BY DESC LIMIT 10 | social | 10 | 17.0 (1.36×b, 6.25×o) | 23.1 (4.60×o) | 106 (0.22×b) |
| Q9 ORDER BY DESC(?age) ?x LIMIT 10 | social | 10 | 9.59 (17.1×b, 157×o) | 164 (9.17×o) | 1,508 (0.11×b) |
| Q10 DISTINCT over a join | social | 9,999 | 160 (5.68×b, 14.6×o) | 910 (2.57×o) | 2,337 (0.39×b) |
| Q11 UNION | social | 127,794 | 269 (1.13×b, 1.85×o) | 305 (1.63×o) | 497 (0.61×b) |
| Q12 MINUS | social | 14,877 | 45.6 (1.75×b, 3.69×o) | 79.8 (2.11×o) | 168 (0.47×b) |
| Q13 :p1 :knows+ ?y COUNT | social | 1 | 168 (2.62×b, 1.77×o) | 441 (0.68×o) | 298 (1.48×b) |
| Q14 ?x rdf:type/rdfs:subClassOf* bsbm:Product | bsbm | 7,800 | 13.0 (0.64×b, 9.95×o) | 8.35 (15.5×o) | 130 (0.064×b) |
| Q15 point lookup :p123 ?p ?o | social | 8 | 0.123 (0.87×b, 0.67×o) | 0.106 (0.77×o) | 0.082 (1.29×b) |
| Q16 COUNT(*) {?s ?p ?o} | social | 1 | 1.24 (49.5×b, 296×o) | 61.1 (5.99×o) | 366 (0.17×b) |
| Q17 BSBM offers: price range + date filter + ORDER BY price LIMIT 10 | bsbm | 10 | 24.3 (3.28×b, 2.89×o) | 79.7 (0.88×o) | 70.3 (1.13×b) |

Geometric-mean speed-up (>1 = faster): pyoxigraph vs baseline: 0.42× over 16 queries; baseline vs pyoxigraph: 2.37× over 16 queries; native vs pyoxigraph: 6.57× over 16 queries; native vs baseline: 2.75× over 17 queries.

### Result checks

Row counts and order-insensitive checksums (canonical terms, blank nodes masked) must agree across every engine and mode; `ordered` queries also compare the row sequence.

| Query | Status | Rows | Checksum | Cells compared |
|---|---|---:|---|---|
| Q1 star-4 (class + age + city + name) | ok | 20,000 | `20000:47293e8d1fc53e8e:f265996f` | 6 |
| Q2 BSBM product star-6 | ok | 1,606 | `1606:e7d1208417db857a:b19a1bf6` | 6 |
| Q3 chain-2 COUNT | ok | 1 | `1:e0207630980d7637:a40c3d0f` | 6 |
| Q4 selective chain-3 from a constant | ok | 27 | `27:f2cb5a35f9f267f7:2ee42fa2` | 6 |
| Q5 OPTIONAL with a type check inside | ok | 20,634 | `20634:60e223ef0b21a5d8:36c70629` | 4 |
| Q6 FILTER(?a > 90) | ok | 1,393 | `1393:7cb8da533ab46bfa:c7ff3f79` | 6 |
| Q7 REGEX on names | ok | 11,111 | `11111:2af49a4de13175e0:6167e6a4` | 6 |
| Q8 GROUP BY + COUNT + ORDER BY DESC LIMIT 10 | ok (ordered) | 10 | `10:c784467a37bf4f18:8029c88b` | 6 |
| Q9 ORDER BY DESC(?age) ?x LIMIT 10 | ok (ordered) | 10 | `10:9cd7a7596d48a4bd:8f614c05` | 6 |
| Q10 DISTINCT over a join | ok | 9,999 | `9999:50df37efdbc95ec2:37b84aef` | 6 |
| Q11 UNION | ok | 127,794 | `127794:06fd246a13c4ce06:36c70629` | 6 |
| Q12 MINUS | ok | 14,877 | `14877:d274ef8848a93849:6743df4a` | 6 |
| Q13 :p1 :knows+ ?y COUNT | ok | 1 | `1:5f08c32f06b2a3c8:a40c3d0f` | 6 |
| Q14 ?x rdf:type/rdfs:subClassOf* bsbm:Product | ok | 7,800 | `7800:edc85e4d56773de5:6743df4a` | 6 |
| Q15 point lookup :p123 ?p ?o | ok | 8 | `8:e2c391c76420fe04:5767b15e` | 6 |
| Q16 COUNT(*) {?s ?p ?o} | ok | 1 | `1:1635f2be009070d0:a40c3d0f` | 6 |
| Q17 BSBM offers: price range + date filter + ORDER BY price LIMIT 10 | ok (ordered) | 10 | `10:cbc9c1d9be3fc526:791bb46c` | 6 |

### Notes

- pyoxigraph timeout (Q5/count): no answer within 120 s (worker process killed)
- pyoxigraph timeout (Q5/full): skipped: timed out in count mode
