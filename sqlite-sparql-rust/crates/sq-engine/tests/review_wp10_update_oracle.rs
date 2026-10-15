//! Adversarial review (WP10, round 2): an independent oracle for `DELETE/INSERT ... WHERE`.
//!
//! The update fuzzer only compares stores with each other (every store runs the same update
//! executor) and checks the direction of change for DELETE/INSERT WHERE. Here the WHERE
//! pattern is evaluated by sq-oracle (lowered with the operation's USING / WITH dataset), the
//! templates are instantiated by this file (SPARQL 1.1 Update §3.1.3, independently of
//! `update/modify.rs`), and the resulting dataset is compared with what `sq_engine::update`
//! leaves, modulo a bijection of the blank nodes created by the operation.
#[allow(dead_code, unused_imports)]
mod fuzz;

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use spargebra::GraphUpdateOperation;
use spargebra::term::{GraphNamePattern, GroundTermPattern, NamedNodePattern, TermPattern};
use sq_core::plan::QueryOptions;
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_core::testing::Rng;
use sq_core::{Term, TermId};
use sq_engine::UpdateOptions;
use sq_memstore::MemStore;
use sq_oracle::{OracleOptions, OracleOutput};

use fuzz::compare::{same, Free, Outcome, Row};
use fuzz::data::Dataset;
use fuzz::engines::{mem_store, now};
use fuzz::query::Order;
use fuzz::update::{Op, Request};

/// Every quad of the store as a row `[g, s, p, o]` (g None = default graph).
fn quads(store: &MemStore) -> Vec<Row> {
    let dec = |id: TermId| -> Option<Arc<Term>> {
        if id == TermId::DEFAULT_GRAPH { None } else { Some(store.dict().decode_one(id).expect("decodes")) }
    };
    store.quads().into_iter().map(|q| q.iter().map(|id| dec(*id)).collect()).collect()
}

fn key(r: &Row) -> String {
    r.iter().map(|t| t.as_ref().map_or("DEFAULT".to_owned(), |t| t.to_ntriples())).collect::<Vec<_>>().join(" ")
}

fn constant(t: oxrdf::TermRef<'_>) -> Option<Arc<Term>> {
    Some(Arc::new(Term::from_oxrdf(t)))
}

/// A template position: the solution's value of a variable, or a constant.
fn bound(sol: &BTreeMap<String, Arc<Term>>, v: &oxrdf::Variable) -> Option<Arc<Term>> {
    sol.get(v.as_str()).cloned()
}

/// Can `[g, s, p, o]` be a quad (graph IRI or default, subject IRI / blank node, predicate IRI)?
fn legal(q: &[Option<Arc<Term>>; 4], g_default: bool) -> bool {
    let iri = |t: &Option<Arc<Term>>| matches!(t.as_deref(), Some(Term::Iri(_)));
    (g_default || iri(&q[0]))
        && matches!(q[1].as_deref(), Some(Term::Iri(_) | Term::BNode(_)))
        && iri(&q[2])
        && q[3].is_some()
}

/// The expected dataset after one DELETE/INSERT WHERE, or None when the oracle cannot
/// evaluate the WHERE pattern.
fn expected(store: &MemStore, op: &GraphUpdateOperation) -> Option<Vec<Row>> {
    let GraphUpdateOperation::DeleteInsert { delete, insert, using, pattern } = op else {
        return None;
    };
    let (lowered, _) = sq_plan::lower_pattern(pattern, using.as_ref(), None, store, &QueryOptions::default()).ok()?;
    let out = sq_oracle::evaluate_with(&lowered, store, &OracleOptions { now: Some(now()), rng_seed: Some(1) }).ok()?;
    let OracleOutput::Solutions { vars, rows } = out else { return None };
    let mut dels: BTreeSet<String> = BTreeSet::new();
    let mut ins: BTreeMap<String, Row> = BTreeMap::new();
    for (n, row) in rows.iter().enumerate() {
        let sol: BTreeMap<String, Arc<Term>> =
            vars.iter().zip(row).filter_map(|(v, t)| Some((v.clone(), t.clone()?))).collect();
        for t in delete {
            let (g, gd) = match &t.graph_name {
                GraphNamePattern::DefaultGraph => (None, true),
                GraphNamePattern::NamedNode(n) => (constant(n.as_ref().into()), false),
                GraphNamePattern::Variable(v) => (bound(&sol, v), false),
            };
            let term = |p: &GroundTermPattern| match p {
                GroundTermPattern::NamedNode(n) => constant(n.as_ref().into()),
                GroundTermPattern::Literal(l) => constant(l.as_ref().into()),
                GroundTermPattern::Variable(v) => bound(&sol, v),
                #[allow(unreachable_patterns)]
                _ => None,
            };
            let p = match &t.predicate {
                NamedNodePattern::NamedNode(n) => constant(n.as_ref().into()),
                NamedNodePattern::Variable(v) => bound(&sol, v),
            };
            let q = [g, term(&t.subject), p, term(&t.object)];
            if (gd || q[0].is_some()) && legal(&q, gd) {
                dels.insert(key(&q.to_vec()));
            }
        }
        // Blank nodes: one per (solution, template label); a blank node minted by the WHERE
        // clause keeps its identity across solutions.
        let fresh = |label: &str| Some(Arc::new(Term::BNode(format!("tI{n}_{label}").into())));
        let value = |t: Option<Arc<Term>>| match t.as_deref() {
            Some(Term::BNode(l)) if l.starts_with('t') => Some(Arc::new(Term::BNode(format!("tW{l}").into()))),
            _ => t,
        };
        for t in insert {
            let (g, gd) = match &t.graph_name {
                GraphNamePattern::DefaultGraph => (None, true),
                GraphNamePattern::NamedNode(n) => (constant(n.as_ref().into()), false),
                GraphNamePattern::Variable(v) => (value(bound(&sol, v)), false),
            };
            let term = |p: &TermPattern| match p {
                TermPattern::NamedNode(n) => constant(n.as_ref().into()),
                TermPattern::Literal(l) => constant(l.as_ref().into()),
                TermPattern::BlankNode(b) => fresh(b.as_str()),
                TermPattern::Variable(v) => value(bound(&sol, v)),
                #[allow(unreachable_patterns)]
                _ => None,
            };
            let p = match &t.predicate {
                NamedNodePattern::NamedNode(n) => constant(n.as_ref().into()),
                NamedNodePattern::Variable(v) => value(bound(&sol, v)),
            };
            let q = [g, term(&t.subject), p, term(&t.object)];
            if (gd || q[0].is_some()) && legal(&q, gd) {
                let r = q.to_vec();
                ins.insert(key(&r), r);
            }
        }
    }
    let mut result: BTreeMap<String, Row> =
        quads(store).into_iter().map(|r| (key(&r), r)).filter(|(k, _)| !dels.contains(k)).collect();
    result.extend(ins);
    Some(result.into_values().collect())
}

/// Runs `op` (a one-operation request) through the oracle and through `sq_engine::update` on
/// a fresh MemStore holding `data`; Some(difference) when they disagree.
fn check(data: &Dataset, req: &Request) -> Option<String> {
    let text = req.to_string();
    let update = sq_plan::parse_update(&text, None).ok()?;
    let [op] = update.operations.as_slice() else { return None };
    let store = mem_store(data);
    let before: BTreeSet<String> = quads(&store)
        .iter()
        .flatten()
        .filter_map(|t| match t.as_deref() {
            Some(Term::BNode(l)) => Some(l.to_string()),
            _ => None,
        })
        .collect();
    let expected = expected(&store, op)?;
    let w: Rc<dyn QuadStoreWriter> = Rc::new(store.clone());
    let uopts = UpdateOptions { now: Some(now()), ..UpdateOptions::default() };
    if let Err(e) = sq_engine::update_with(w, &update, &req.options(), &uopts) {
        return Some(format!("update failed: {e}\n{text}"));
    }
    // Blank nodes the operation created are free.
    let actual: Vec<Row> = quads(&store)
        .into_iter()
        .map(|r| {
            r.into_iter()
                .map(|t| match t.as_deref() {
                    Some(Term::BNode(l)) if !before.contains(&**l) => Some(Arc::new(Term::BNode(format!("tA{l}").into()))),
                    _ => t,
                })
                .collect()
        })
        .collect();
    same(&Outcome::Graph(expected.clone()), &Outcome::Graph(actual.clone()), Order::Set, Free::Minted).err().map(|d| {
        format!(
            "{d}\n--- data:\n{}\n--- update:\n{text}--- expected:\n{}--- actual:\n{}",
            data.trig(),
            Outcome::Graph(expected).describe(Order::Set, 60),
            Outcome::Graph(actual).describe(Order::Set, 60)
        )
    })
}

/// Random DELETE/INSERT WHERE and DELETE WHERE operations (the update fuzzer's generator,
/// under random join policies), each checked against the oracle instantiation.
#[test]
fn review_modify_agrees_with_an_independent_oracle_instantiation() {
    let cases: usize = std::env::var("SQ_REVIEW_UPDATE_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(1500);
    let seed: u64 = std::env::var("SQ_REVIEW_UPDATE_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(11);
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut failures = Vec::new();
    let mut checked = 0;
    for _ in 0..cases {
        let data = Dataset::generate(&mut r);
        let req = Request::generate(&mut r, &data);
        for op in &req.ops {
            if !matches!(op, Op::Modify { .. } | Op::DeleteWhere(_)) {
                continue;
            }
            let one = Request { ops: vec![op.clone()], ..req.clone() };
            checked += 1;
            if let Some(d) = check(&data, &one) {
                failures.push(d);
            }
        }
    }
    eprintln!("{checked} operations checked, {} failures", failures.len());
    assert!(failures.is_empty(), "{} of {checked} operations disagree; first:\n{}", failures.len(), failures[0]);
}
