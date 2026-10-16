## sparqlite benchmark

Run 2026-10-16T01:55:47+00:00 on Intel(R) Xeon(R) Processor @ 2.80GHz (4 CPUs, 16 GiB); SQLite 3.45.1, Python 3.13.16, pyoxigraph 0.5.11, rdflib 7.6.0, sparqlite 0.1.0 (SQLite 3.45.1); git 117d39281c88; harness `91e26652b40e`.

Settings: modes count, full; median of 5 after 1 warm-up; timeout 120 s; cell budget 120 s; json mode off (`sparql_query` and the baseline's `sparql_query_sql` not measured); full columns `terms`; reference count `aggregate`; native config `reader=sql`; SQLite load `auto` (databases reused); pyoxigraph/rdflib in worker processes; rdflib up to 100,000 triples. Command: `py/bench.py --social $SCRATCH/data/social-200k.nt --bsbm $SCRATCH/data/bsbm-1m.nt --engines native --modes count,full --native-config reader=sql --ext $SCRATCH/targets/ext/release/libsparqlite --work-dir $SCRATCH/work/benchFinal --reuse-db --timeout 120 --out-md $SCRATCH/work/benchFinal/results-sqlreader.md --out-json $SCRATCH/work/benchFinal/results-sqlreader.json`.

Speed-ups in parentheses are the other engine's time divided by this engine's (>1 = this engine is faster): `×b` vs sparqlite-baseline, `×o` vs pyoxigraph. † = fewer repetitions than requested (cell budget 120 s).

### Datasets and load times

| Dataset | Triples | File MB | SHA-256 | native[reader=sql] |
|---|---:|---:|---|---:|
| social | 1,600,000 | 175.0 | `b0c8f3a5adeb` | 10.46 s, 129 MB |
| bsbm | 1,004,103 | 146.9 | `6586dcc57a51` | 7.08 s, 90 MB |

- sparqlite-native on social: load 9.06 s (file), ANALYZE 0.76 s, sparql_analyze 0.64 s (database reused from an earlier run)
- sparqlite-native on bsbm: load 6.12 s (file), ANALYZE 0.54 s, sparql_analyze 0.42 s (database reused from an earlier run)

### count mode (solutions counted inside each engine, no result term materialised)

Median ms of 5 runs after 1 warm-up. native[reader=sql]: `sparql_count(q)`.

| Query | Data | Rows | native[reader=sql] |
|---|---|---:|---:|
| Q1 star-4 (class + age + city + name) | social | 20,000 | 49.7 |
| Q2 BSBM product star-6 | bsbm | 1,606 | 6.87 |
| Q3 chain-2 COUNT | social | 1 | 182 |
| Q4 selective chain-3 from a constant | social | 27 | 0.113 |
| Q5 OPTIONAL with a type check inside | social | 20,634 | 36.6 |
| Q6 FILTER(?a > 90) | social | 1,393 | 0.299 |
| Q7 REGEX on names | social | 11,111 | 96.2 |
| Q8 GROUP BY + COUNT + ORDER BY DESC LIMIT 10 | social | 10 | 39.8 |
| Q9 ORDER BY DESC(?age) ?x LIMIT 10 | social | 10 | 33.3 |
| Q10 DISTINCT over a join | social | 9,999 | 310 |
| Q11 UNION | social | 127,794 | 55.8 |
| Q12 MINUS | social | 14,877 | 34.0 |
| Q13 :p1 :knows+ ?y COUNT | social | 1 | 267 |
| Q14 ?x rdf:type/rdfs:subClassOf* bsbm:Product | bsbm | 7,800 | 50.3 |
| Q15 point lookup :p123 ?p ?o | social | 8 | 0.060 |
| Q16 COUNT(*) {?s ?p ?o} | social | 1 | 65.5 |
| Q17 BSBM offers: price range + date filter + ORDER BY price LIMIT 10 | bsbm | 10 | 44.7 |

### full mode (one Python string per projected result term, on every engine)

Median ms of 5 runs after 1 warm-up. native[reader=sql]: `SELECT t0..t{n-1} FROM sparql_rows(q)`.

| Query | Data | Rows | native[reader=sql] |
|---|---|---:|---:|
| Q1 star-4 (class + age + city + name) | social | 20,000 | 102 |
| Q2 BSBM product star-6 | bsbm | 1,606 | 13.1 |
| Q3 chain-2 COUNT | social | 1 | 184 |
| Q4 selective chain-3 from a constant | social | 27 | 0.139 |
| Q5 OPTIONAL with a type check inside | social | 20,634 | 58.7 |
| Q6 FILTER(?a > 90) | social | 1,393 | 2.21 |
| Q7 REGEX on names | social | 11,111 | 131 |
| Q8 GROUP BY + COUNT + ORDER BY DESC LIMIT 10 | social | 10 | 39.8 |
| Q9 ORDER BY DESC(?age) ?x LIMIT 10 | social | 10 | 33.4 |
| Q10 DISTINCT over a join | social | 9,999 | 305 |
| Q11 UNION | social | 127,794 | 294 |
| Q12 MINUS | social | 14,877 | 53.8 |
| Q13 :p1 :knows+ ?y COUNT | social | 1 | 284 |
| Q14 ?x rdf:type/rdfs:subClassOf* bsbm:Product | bsbm | 7,800 | 60.1 |
| Q15 point lookup :p123 ?p ?o | social | 8 | 0.071 |
| Q16 COUNT(*) {?s ?p ?o} | social | 1 | 65.2 |
| Q17 BSBM offers: price range + date filter + ORDER BY price LIMIT 10 | bsbm | 10 | 46.8 |

### Result checks

Row counts and order-insensitive checksums (canonical terms, blank nodes masked) must agree across every engine and mode; `ordered` queries also compare the row sequence.

| Query | Status | Rows | Checksum | Cells compared |
|---|---|---:|---|---|
| Q1 star-4 (class + age + city + name) | single engine | 20,000 | `20000:47293e8d1fc53e8e:f265996f` | 2 |
| Q2 BSBM product star-6 | single engine | 1,606 | `1606:e7d1208417db857a:b19a1bf6` | 2 |
| Q3 chain-2 COUNT | single engine | 1 | `1:e0207630980d7637:a40c3d0f` | 2 |
| Q4 selective chain-3 from a constant | single engine | 27 | `27:f2cb5a35f9f267f7:2ee42fa2` | 2 |
| Q5 OPTIONAL with a type check inside | single engine | 20,634 | `20634:60e223ef0b21a5d8:36c70629` | 2 |
| Q6 FILTER(?a > 90) | single engine | 1,393 | `1393:7cb8da533ab46bfa:c7ff3f79` | 2 |
| Q7 REGEX on names | single engine | 11,111 | `11111:2af49a4de13175e0:6167e6a4` | 2 |
| Q8 GROUP BY + COUNT + ORDER BY DESC LIMIT 10 | single engine | 10 | `10:c784467a37bf4f18:8029c88b` | 2 |
| Q9 ORDER BY DESC(?age) ?x LIMIT 10 | single engine | 10 | `10:9cd7a7596d48a4bd:8f614c05` | 2 |
| Q10 DISTINCT over a join | single engine | 9,999 | `9999:50df37efdbc95ec2:37b84aef` | 2 |
| Q11 UNION | single engine | 127,794 | `127794:06fd246a13c4ce06:36c70629` | 2 |
| Q12 MINUS | single engine | 14,877 | `14877:d274ef8848a93849:6743df4a` | 2 |
| Q13 :p1 :knows+ ?y COUNT | single engine | 1 | `1:5f08c32f06b2a3c8:a40c3d0f` | 2 |
| Q14 ?x rdf:type/rdfs:subClassOf* bsbm:Product | single engine | 7,800 | `7800:edc85e4d56773de5:6743df4a` | 2 |
| Q15 point lookup :p123 ?p ?o | single engine | 8 | `8:e2c391c76420fe04:5767b15e` | 2 |
| Q16 COUNT(*) {?s ?p ?o} | single engine | 1 | `1:1635f2be009070d0:a40c3d0f` | 2 |
| Q17 BSBM offers: price range + date filter + ORDER BY price LIMIT 10 | single engine | 10 | `10:cbc9c1d9be3fc526:791bb46c` | 2 |
