use sq_core::ir::Expr;
use sq_core::spargebra::algebra::Function;

use super::sql;
use crate::helpers::{
    FALSE_ID, HELPER_FUNCTIONS, HelperKind, TRUE_ID, aggregate_name, cast_name, eval_template, function_name, helper,
};

#[test]
fn boolean_ids_are_the_inline_bools() {
    assert_eq!(TRUE_ID, sq_core::TermId::from_bool(true).raw());
    assert_eq!(FALSE_ID, sq_core::TermId::from_bool(false).raw());
    assert_eq!((TRUE_ID, FALSE_ID), (23, 7));
}

#[test]
fn operator_templates() {
    let s = |i| Box::new(Expr::Slot(i));
    assert_eq!(eval_template("<=", 2), Some(Expr::LessOrEqual(s(0), s(1))));
    assert_eq!(eval_template("/", 2), Some(Expr::Divide(s(0), s(1))));
    assert_eq!(eval_template("u-", 1), Some(Expr::UnaryMinus(s(0))));
    assert_eq!(eval_template("=", 1), None, "operators have fixed arity");
    assert_eq!(eval_template("u+", 2), None);
    assert_eq!(eval_template("no such op", 1), None);
    assert_eq!(
        eval_template("SUBSTR", 3),
        Some(Expr::Call(Function::SubStr, vec![Expr::Slot(0), Expr::Slot(1), Expr::Slot(2)]))
    );
    assert_eq!(eval_template("CONCAT", 0), Some(Expr::Call(Function::Concat, vec![])));
}

#[test]
fn builtin_names_round_trip() {
    let deterministic = [
        Function::Str, Function::Lang, Function::LangMatches, Function::Datatype, Function::Iri, Function::Abs,
        Function::Ceil, Function::Floor, Function::Round, Function::Concat, Function::SubStr, Function::StrLen,
        Function::Replace, Function::UCase, Function::LCase, Function::EncodeForUri, Function::Contains,
        Function::StrStarts, Function::StrEnds, Function::StrBefore, Function::StrAfter, Function::Year,
        Function::Month, Function::Day, Function::Hours, Function::Minutes, Function::Seconds, Function::Timezone,
        Function::Tz, Function::Md5, Function::Sha1, Function::Sha256, Function::Sha384, Function::Sha512,
        Function::StrLang, Function::StrDt, Function::IsIri, Function::IsBlank, Function::IsLiteral,
        Function::IsNumeric, Function::Regex,
    ];
    let mut names = std::collections::HashSet::new();
    for f in deterministic {
        let name = function_name(&f).unwrap_or_else(|| panic!("{f} has no name"));
        assert!(names.insert(name), "{name} is used twice");
        assert_eq!(eval_template(name, 1), Some(Expr::Call(f, vec![Expr::Slot(0)])));
    }
    for f in [Function::BNode, Function::Rand, Function::Now, Function::Uuid, Function::StrUuid] {
        assert_eq!(function_name(&f), None, "{f} is not deterministic per call");
    }
}

#[test]
fn casts_are_named_by_their_xsd_datatype() {
    let int = "http://www.w3.org/2001/XMLSchema#integer";
    assert_eq!(cast_name(int), Some(int));
    assert!(cast_name("http://www.w3.org/2001/XMLSchema#gMonthDay").is_some());
    assert_eq!(cast_name("http://www.w3.org/2001/XMLSchema#notAType"), None);
    assert_eq!(cast_name("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"), None);
    assert_eq!(cast_name("http://ex.org/f"), None);
    let Some(Expr::Call(Function::Custom(n), args)) = eval_template(int, 1) else { panic!("cast template") };
    assert_eq!((n.as_str(), args), (int, vec![Expr::Slot(0)]));
}

#[test]
fn the_helper_list_is_consistent() {
    let mut names = std::collections::HashSet::new();
    for h in HELPER_FUNCTIONS {
        assert!(names.insert(h.name), "{} listed twice", h.name);
        assert!(h.name.starts_with("sparql_") && h.min_args <= h.max_args && !h.semantics.is_empty());
    }
    use sq_core::ir::AggFunc;
    for f in [AggFunc::Sum, AggFunc::Avg, AggFunc::Min, AggFunc::Max, AggFunc::Sample, AggFunc::GroupConcat {
        separator: ",".into(),
    }] {
        let name = aggregate_name(&f).expect("aggregate helper");
        assert_eq!(helper(name).map(|h| h.kind), Some(HelperKind::Aggregate), "{name}");
    }
    assert_eq!(aggregate_name(&AggFunc::Count), None, "COUNT is plain SQL");
}

/// Every function call and `sparql_eval` operator in generated SQL is part of the contract.
#[test]
fn generated_sql_only_uses_listed_helpers() {
    let queries = [
        "SELECT ?x WHERE { ?x :age ?a FILTER(?a > 1 && ?a < 2.5 && ?a != ?x && ?a <= ?x) }",
        "SELECT ?x WHERE { ?x :name ?n FILTER(REGEX(?n, \"a\") && STRSTARTS(?n, \"A\") && STRENDS(?n, \"e\")) }",
        "SELECT ?x ?y WHERE { ?x :age ?a BIND(xsd:string(?a) AS ?y) BIND(-?a * 2 / 3 AS ?z) \
         BIND(UCASE(STR(?x)) AS ?u) }",
        "SELECT ?x (SUM(?a) AS ?s) (AVG(?a) AS ?m) (MIN(?a) AS ?lo) (MAX(?a) AS ?hi) (SAMPLE(?a) AS ?e) \
         (GROUP_CONCAT(?a) AS ?c) WHERE { ?x :age ?a } GROUP BY ?x ORDER BY ?x",
        "SELECT ?x WHERE { ?x :score ?s FILTER(?s) FILTER(isNumeric(?s) || LANGMATCHES(LANG(?s), \"en\")) }",
    ];
    let sql_builtins =
        ["COUNT", "COALESCE", "EXISTS", "instr", "substr", "length", "row_number", "min", "VALUES", "IN"];
    let keywords = ["AND", "OR", "NOT", "AS", "THEN", "ELSE", "WHEN"];
    for q in queries {
        let text = sql(q).sql;
        let bytes = text.as_bytes();
        for (i, _) in text.match_indices('(') {
            let ident = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_';
            let start = bytes[..i].iter().rposition(|b| !ident(b)).map_or(0, |p| p + 1);
            let name = &text[start..i];
            if name.is_empty() || sql_builtins.contains(&name) || keywords.contains(&name) {
                continue;
            }
            assert!(helper(name).is_some(), "{name} is not a listed helper ({q})");
        }
        for (i, _) in text.match_indices("sparql_eval('") {
            let rest = &text[i + "sparql_eval('".len()..];
            let op = &rest[..rest.find('\'').unwrap()];
            assert!((0..=3).any(|n| eval_template(op, n).is_some()), "unknown sparql_eval op {op}");
        }
    }
}
