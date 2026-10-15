//! Lowering tests: slot layout and scopes, blank nodes, constants, datasets, forms, update
//! WHERE patterns.
mod common;

use common::*;
use sq_core::id::Tag;
use sq_core::ir::{
    AggFunc, DefaultGraph, GraphSel, Logical, NamedGraphs, PatTerm, QueryForm, TemplateTerm, VarOrigin,
};
use sq_core::plan::QueryOptions;
use sq_core::TermId;
use sq_memstore::MemStore;

fn slot_of(q: &sq_core::ir::LoweredQuery, name: &str) -> Vec<u16> {
    q.vars.vars.iter().enumerate().filter(|(_, (n, _))| n == name).map(|(i, _)| i as u16).collect()
}

fn all_quads(l: &Logical, out: &mut Vec<sq_core::ir::QuadPat>) {
    match l {
        Logical::Bgp(p) => out.extend(p.iter().copied()),
        Logical::Join(a, b) | Logical::Union(a, b) => {
            all_quads(a, out);
            all_quads(b, out);
        }
        Logical::LeftJoin { left, right, .. } | Logical::Minus { left, right } => {
            all_quads(left, out);
            all_quads(right, out);
        }
        Logical::Filter { input, .. }
        | Logical::Extend { input, .. }
        | Logical::Graph { input, .. }
        | Logical::Group { input, .. }
        | Logical::OrderBy { input, .. }
        | Logical::Project { input, .. }
        | Logical::Distinct(input)
        | Logical::Reduced(input)
        | Logical::Slice { input, .. } => all_quads(input, out),
        _ => {}
    }
}

#[test]
fn sub_select_scopes_are_alpha_renamed() {
    let st = MemStore::new();
    let q = lower("SELECT ?x ?y { ?x :p ?y { SELECT ?x { ?x :q ?y } } }", &st);
    let ys = slot_of(&q, "y");
    assert_eq!(ys.len(), 2, "outer and inner ?y are different slots");
    assert_eq!(q.vars.vars[ys[0] as usize].1, VarOrigin::User);
    assert_eq!(q.vars.vars[ys[1] as usize].1, VarOrigin::Renamed);
    assert_eq!(slot_of(&q, "x").len(), 1, "the projected ?x is shared");
    assert_eq!(q.form, QueryForm::Select(vec![slot_of(&q, "x")[0], ys[0]]));
    // The inner Project keeps only ?x.
    let Logical::Project { input, .. } = &q.root else { panic!() };
    let Logical::Join(_, inner) = &**input else { panic!("{:?}", q.root) };
    assert!(matches!(&**inner, Logical::Project { slots, .. } if slots == &slot_of(&q, "x")));
    // EXISTS shares the layout with its enclosing scope.
    let q = lower("SELECT * { ?x :p ?y FILTER EXISTS { ?x :q ?z } }", &st);
    assert_eq!(slot_of(&q, "x").len(), 1);
    assert_eq!(q.exists.len(), 1);
}

#[test]
fn pattern_blank_nodes_are_hidden_slots() {
    let st = MemStore::new();
    let q = lower("SELECT * { _:a :p ?x . _:a :q ?y . [] :r ?z . ?w :s [ :t ?v ] }", &st);
    let hidden: Vec<usize> =
        q.vars.vars.iter().enumerate().filter(|(_, (_, o))| *o == VarOrigin::BlankNode).map(|(i, _)| i).collect();
    assert_eq!(hidden.len(), 3, "_:a once, [] and the property-list node: {:?}", q.vars);
    let QueryForm::Select(proj) = &q.form else { panic!() };
    assert!(proj.iter().all(|s| !hidden.contains(&(*s as usize))), "blank nodes are never projected");
    let mut quads = Vec::new();
    all_quads(&q.root, &mut quads);
    let subj_a: Vec<PatTerm> = quads.iter().take(2).map(|p| p.s).collect();
    assert_eq!(subj_a[0], subj_a[1], "_:a is one slot");
    // A split property path shares its middle node.
    let q = lower("SELECT * { ?s :p/:q ?o }", &st);
    let mut quads = Vec::new();
    all_quads(&q.root, &mut quads);
    assert_eq!(quads.len(), 2);
    assert_eq!(quads[0].o, quads[1].s);
}

#[test]
fn constants_resolve_to_store_ids_or_temps() {
    let st = store(":a :p \"a long string literal\" .");
    let q = lower("SELECT * { :a :p ?o . ?s :absent 42 . ?s :p \"a long string literal\" }", &st);
    let mut quads = Vec::new();
    all_quads(&q.root, &mut quads);
    let PatTerm::Const(a) = quads[0].s else { panic!() };
    assert_eq!(a.tag(), Tag::Iri);
    let PatTerm::Const(absent) = quads[1].p else { panic!() };
    assert!(absent.is_temp() && absent.temp_tag() == Tag::Iri);
    assert_eq!(quads[1].o, PatTerm::Const(TermId::from_int(42).unwrap()), "inline literal");
    let PatTerm::Const(lit) = quads[2].o else { panic!() };
    assert_eq!(lit.tag(), Tag::Str);
    assert_eq!(q.temps.len(), 1);
}

#[test]
fn values_graph_and_aggregates() {
    let st = MemStore::new();
    let q = lower("SELECT * { VALUES (?x ?y) { (1 UNDEF) (UNDEF 2) } }", &st);
    let Logical::Project { input, .. } = &q.root else { panic!() };
    let Logical::Values { rows, .. } = &**input else { panic!() };
    assert_eq!(rows[0][1], TermId::UNBOUND);
    assert_eq!(rows[1][0], TermId::UNBOUND);

    let q = lower("SELECT * { GRAPH ?g { ?s ?p ?o } ?s ?p ?o }", &st);
    let mut quads = Vec::new();
    all_quads(&q.root, &mut quads);
    assert_eq!(quads[0].g, GraphSel::Active);
    assert_eq!(quads[1].g, GraphSel::Default);

    let q = lower("SELECT ?k (SUM(DISTINCT ?v) AS ?s) (COUNT(*) AS ?n) { ?x :p ?v } GROUP BY (STR(?x) AS ?k)", &st);
    let mut group = None;
    fn find<'a>(l: &'a Logical, out: &mut Option<&'a Logical>) {
        if let Logical::Group { .. } = l {
            *out = Some(l);
        }
        for c in match l {
            Logical::Extend { input, .. } | Logical::Project { input, .. } | Logical::Filter { input, .. } => {
                vec![&**input]
            }
            _ => vec![],
        } {
            find(c, out);
        }
    }
    find(&q.root, &mut group);
    let Some(Logical::Group { input, keys, aggs }) = group else { panic!("{:?}", q.root) };
    assert_eq!(keys, &slot_of(&q, "k"));
    assert!(matches!(&**input, Logical::Extend { .. }), "GROUP BY expressions are Extends below the Group");
    assert_eq!(aggs.len(), 2);
    assert!(aggs.iter().any(|(_, a)| a.func == AggFunc::Sum && a.distinct));
    assert!(aggs.iter().any(|(_, a)| a.func == AggFunc::CountAll && a.arg.is_none()));
    assert!(aggs.iter().all(|(s, _)| q.vars.vars[*s as usize].1 == VarOrigin::Generated));
}

#[test]
fn dataset_clauses() {
    let st = store(":a :p :b .");
    let q = lower("SELECT * { ?s ?p ?o }", &st);
    assert_eq!(q.dataset.default, DefaultGraph::StoreDefault);
    assert_eq!(q.dataset.named, NamedGraphs::All);
    let q = lower_with("SELECT * { ?s ?p ?o }", &st, &QueryOptions { union_default_graph: true, ..Default::default() });
    assert_eq!(q.dataset.default, DefaultGraph::UnionAll);
    let q = lower("SELECT * FROM :g1 FROM :g1 { ?s ?p ?o }", &st);
    let DefaultGraph::Graphs(g) = &q.dataset.default else { panic!() };
    assert_eq!(g.len(), 1, "duplicates collapse");
    assert!(g[0].is_temp(), "graphs absent from the store are TEMP ids (empty graphs)");
    assert_eq!(q.dataset.named, NamedGraphs::Only(Vec::new().into()));
    let q = lower("SELECT * FROM NAMED :g1 FROM NAMED :g2 { ?s ?p ?o }", &st);
    assert_eq!(q.dataset.default, DefaultGraph::Graphs(Vec::new().into()), "FROM NAMED only: empty default graph");
    assert!(matches!(&q.dataset.named, NamedGraphs::Only(l) if l.len() == 2));
}

#[test]
fn forms() {
    let st = MemStore::new();
    let q = lower("CONSTRUCT { ?s :q _:b . _:b :r ?o . _:c :r :x } WHERE { ?s :p ?o }", &st);
    let QueryForm::Construct(t) = &q.form else { panic!() };
    assert_eq!(t.len(), 3);
    assert_eq!(t[0][2], TemplateTerm::BNode(0));
    assert_eq!(t[1][0], TemplateTerm::BNode(0));
    assert_eq!(t[2][0], TemplateTerm::BNode(1));
    assert_eq!(t[0][0], TemplateTerm::Slot(slot_of(&q, "s")[0]));
    let q = lower("DESCRIBE :x ?y { ?y :p :x }", &st);
    let QueryForm::Describe(slots) = &q.form else { panic!() };
    assert_eq!(slots.len(), 2);
    assert_eq!(q.vars.vars[slots[0] as usize].1, VarOrigin::Generated, "DESCRIBE <iri> binds a generated slot");
    assert_eq!(lower("ASK { ?s ?p ?o }", &st).form, QueryForm::Ask);
}

#[test]
fn base_iri_and_too_many_variables() {
    let st = MemStore::new();
    let parsed = sq_plan::parse_query("SELECT * { <a> <b> ?c }", Some("http://base.example/")).unwrap();
    let q = sq_plan::lower_query(&parsed, &st, &QueryOptions::default()).unwrap();
    assert_eq!(q.base_iri.as_deref(), Some("http://base.example/"));
    assert!(matches!(sq_plan::parse_query("SELECT * {", None), Err(sq_core::Error::Syntax(_))));
    let many: String = (0..300).map(|i| format!("?s{i} :p ?o{i} . ")).collect();
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}SELECT * {{ {many} }}"), None).unwrap();
    assert!(matches!(sq_plan::lower_query(&parsed, &st, &QueryOptions::default()), Err(sq_core::Error::Unsupported(_))));
}

#[test]
fn update_where_with_using_and_with() {
    let st = store(":a :p :b .");
    let u = sq_plan::parse_update(
        "PREFIX : <http://ex/> WITH :g DELETE { ?s ?p ?o } INSERT { ?s :q ?o } WHERE { ?s ?p ?o OPTIONAL { ?o :r ?z } }",
        None,
    )
    .unwrap();
    let spargebra::GraphUpdateOperation::DeleteInsert { using, pattern, .. } = &u.operations[0] else { panic!() };
    let (q, map) = sq_plan::lower_pattern(pattern, using.as_ref(), None, &st, &QueryOptions::default()).unwrap();
    let DefaultGraph::Graphs(g) = &q.dataset.default else { panic!("{:?}", q.dataset) };
    assert_eq!(g.len(), 1);
    assert_eq!(q.dataset.named, NamedGraphs::All, "WITH keeps every named graph");
    for v in ["s", "p", "o", "z"] {
        assert!(map.contains_key(&spargebra::term::Variable::new_unchecked(v)), "{v} in map");
    }
    let QueryForm::Select(in_scope) = &q.form else { panic!() };
    assert_eq!(in_scope.len(), 4);
    let p = sq_plan::optimize(q, &st, &QueryOptions::default()).unwrap();
    sq_plan::validate(&p).unwrap();

    let u = sq_plan::parse_update(
        "PREFIX : <http://ex/> DELETE { ?s ?p ?o } USING :a USING NAMED :b WHERE { GRAPH ?g { ?s ?p ?o } }",
        None,
    )
    .unwrap();
    let spargebra::GraphUpdateOperation::DeleteInsert { using, pattern, .. } = &u.operations[0] else { panic!() };
    let (q, _) = sq_plan::lower_pattern(pattern, using.as_ref(), None, &st, &QueryOptions::default()).unwrap();
    assert!(matches!(&q.dataset.default, DefaultGraph::Graphs(g) if g.len() == 1));
    assert!(matches!(&q.dataset.named, NamedGraphs::Only(g) if g.len() == 1));
    // No USING / WITH: the query defaults.
    let u = sq_plan::parse_update("DELETE WHERE { ?s ?p ?o }", None).unwrap();
    let spargebra::GraphUpdateOperation::DeleteInsert { using, pattern, .. } = &u.operations[0] else { panic!() };
    let (q, _) = sq_plan::lower_pattern(pattern, using.as_ref(), None, &st, &QueryOptions::default()).unwrap();
    assert_eq!(q.dataset.default, DefaultGraph::StoreDefault);
}

#[test]
fn logical_snapshot() {
    let st = store(":alice :knows :bob .");
    let q = lower(
        "SELECT ?x (COUNT(?y) AS ?n) WHERE { ?x :knows ?y OPTIONAL { ?y :age ?a FILTER(?a > 30) } \
         FILTER NOT EXISTS { ?y :blocked ?x } } GROUP BY ?x ORDER BY DESC(?n) LIMIT 5",
        &st,
    );
    let text = sq_plan::explain_logical_with(&q, Some(&st));
    let expected = "\
Select [?x ?n]
Dataset default=store-default named=all
Slice offset=0 limit=5
  Project [?x ?n]
    OrderBy [DESC(?n)]
      Extend ?n := ?_3
        Group keys=[?x] aggs=[?_3=count(?y)]
          Filter !EXISTS#0
            LeftJoin filter=(?a > 30)
              Bgp
                ?x <http://ex/knows> ?y
              Bgp
                ?y <http://ex/age> ?a
EXISTS #0
  Bgp
    ?y <http://ex/blocked> ?x
";
    assert_eq!(text, expected, "\n{text}");
}
