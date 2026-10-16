//! The 17 benchmark queries (py/queries/*.rq) on small deterministic datasets with the same
//! vocabulary as py/gen_social.py and py/gen_bsbm.py: which ones the baseline supports, and
//! that each supported one gives spareval's answer.
mod common;

use std::fmt::Write as _;
use std::path::Path;

use common::{DataFile, Engine, check};

/// xorshift64*: deterministic data without extra dependencies.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// Social graph: every person has a class, a name, an age, three `:knows`, a city and a join
/// date (8 triples), as in gen_social.py.
fn social(persons: u64) -> String {
    let mut rng = Rng(0x5eed_0001);
    let mut out = String::from(
        "@prefix : <http://example.org/social/> .\n@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n",
    );
    for i in 0..persons {
        // Ages skew young with a thin tail above 90 (FILTER(?a > 90) stays selective).
        let age = if rng.below(10) == 0 { 85 + rng.below(15) } else { rng.below(60) };
        let city = rng.below(7);
        let day = 1 + rng.below(28);
        let _ = writeln!(
            out,
            ":p{i} rdf:type :C{} ; :name \"Person {i}\" ; :age {age} ; :city :city{city} ; \
             :joined \"2015-03-{day:02}\"^^<{XSD}date> .",
            i % 10
        );
        let mut friends: Vec<u64> = Vec::new();
        while friends.len() < 3 {
            // A popular head and a long tail.
            let j = if rng.below(3) == 0 { rng.below(5) } else { rng.below(persons) };
            if j != i && !friends.contains(&j) {
                friends.push(j);
            }
        }
        for j in friends {
            let _ = writeln!(out, ":p{i} :knows :p{j} .");
        }
    }
    out
}

/// BSBM-lite: a 3-level product-type hierarchy under bsbm:Product, features, producers,
/// vendors, products and offers whose prices mix two-decimal xsd:decimal (some non-canonical,
/// e.g. "12.50") with xsd:integer, and validTo dateTimes without time zone.
fn bsbm(products: u64) -> String {
    let mut rng = Rng(0x5eed_0002);
    let mut out = String::from(
        "@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n\
         @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
         @prefix bsbm: <http://example.org/bsbm/vocabulary/> .\n\
         @prefix inst: <http://example.org/bsbm/instances/> .\n",
    );
    // Types 1..=2 under Product, 3..=6 under them, 7..=14 leaves.
    for t in 1..=14u64 {
        let parent = if t <= 2 { "bsbm:Product".to_owned() } else { format!("inst:ProductType{}", (t - 1) / 2) };
        let _ = writeln!(out, "inst:ProductType{t} rdf:type bsbm:ProductType ; rdfs:subClassOf {parent} .");
    }
    for f in 1..=12 {
        let _ = writeln!(out, "inst:ProductFeature{f} rdf:type bsbm:ProductFeature ; rdfs:label \"feature {f}\" .");
    }
    for p in 1..=products {
        let ptype = 7 + rng.below(8);
        let _ = writeln!(
            out,
            "inst:Product{p} rdf:type inst:ProductType{ptype} ; rdfs:label \"product {p}\" ; \
             bsbm:producer inst:Producer{} ; bsbm:productPropertyNumeric1 {} ; \
             bsbm:productPropertyTextual1 \"text {}\" .",
            1 + rng.below(4),
            1 + rng.below(2000),
            rng.below(50)
        );
        let mut features = vec![3];
        for _ in 0..3 {
            features.push(1 + rng.below(12));
        }
        features.sort_unstable();
        features.dedup();
        for f in features.into_iter().filter(|_| rng.below(3) != 0 || p % 2 == 0) {
            let _ = writeln!(out, "inst:Product{p} bsbm:productFeature inst:ProductFeature{f} .");
        }
    }
    for o in 1..=products * 4 {
        let price = match rng.below(7) {
            0 => format!("{}", 400 + rng.below(800)),
            1 => format!("\"{}.50\"^^<{XSD}decimal>", 400 + rng.below(800)),
            _ => format!("{}.{:02}", 300 + rng.below(1000), 1 + rng.below(98)),
        };
        let _ = writeln!(
            out,
            "inst:Offer{o} rdf:type bsbm:Offer ; bsbm:product inst:Product{} ; bsbm:vendor inst:Vendor{} ; \
             bsbm:price {price} ; bsbm:validTo \"2008-{:02}-{:02}T{:02}:00:00\"^^<{XSD}dateTime> .",
            1 + rng.below(products),
            1 + rng.below(3),
            1 + rng.below(12),
            1 + rng.below(28),
            rng.below(24)
        );
    }
    out
}

#[test]
fn benchmark_queries_are_supported_and_match_spareval() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../py/queries");
    let Ok(listing) = std::fs::read_dir(&dir) else {
        eprintln!("note: {} not found; skipping", dir.display());
        return;
    };
    let mut files: Vec<_> = listing.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| {
        p.extension().is_some_and(|x| x == "rq")
    }).collect();
    files.sort();
    let social = Engine::new(&[DataFile::turtle(&social(300))]).unwrap();
    let bsbm = Engine::new(&[DataFile::turtle(&bsbm(60))]).unwrap();
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let engine = if text.contains("# dataset: bsbm") { &bsbm } else { &social };
        match check(engine, &text, None) {
            Ok(Some(sql)) => {
                if std::env::var_os("SQ_SQLGEN_PRINT").is_some() {
                    eprintln!("-- {name}\n{}\n", sql.sql);
                }
                supported.push(name)
            }
            Ok(None) => unsupported.push(name),
            Err(e) => panic!("{name}: {e}"),
        }
    }
    eprintln!("baseline supports {}/{} benchmark queries: {}", supported.len(), files.len(), supported.join(" "));
    if !unsupported.is_empty() {
        eprintln!("unsupported: {}", unsupported.join(" "));
    }
    assert_eq!(files.len(), 17, "py/queries holds the 17 benchmark queries");
    assert!(unsupported.is_empty(), "every benchmark query is inside the baseline subset");
}

#[test]
fn generated_data_exercises_the_selective_filters() {
    // The datasets must make the benchmark queries non-trivial (non-empty answers).
    let social = Engine::new(&[DataFile::turtle(&social(300))]).unwrap();
    let bsbm = Engine::new(&[DataFile::turtle(&bsbm(60))]).unwrap();
    let count = |e: &Engine, q: &str| {
        let query = sq_plan::parse_query(q, None).unwrap();
        match e.reference(&query).unwrap() {
            common::Outcome::Solutions { rows, .. } => rows.len(),
            common::Outcome::Boolean(_) => unreachable!("SELECT"),
        }
    };
    assert!(count(&social, "SELECT ?x WHERE { ?x <http://example.org/social/age> ?a FILTER(?a > 90) }") > 0);
    assert!(
        count(
            &bsbm,
            "PREFIX bsbm: <http://example.org/bsbm/vocabulary/> SELECT ?o WHERE { ?o bsbm:price ?p \
             FILTER(?p >= 500 && ?p <= 1000) }"
        ) > 10
    );
}
