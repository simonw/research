use crate::codec::*;
use crate::id::{Kind, Tag, TermId};
use crate::store::{IndexOrder, QuadStore};
use crate::testing::{cursor_conformance, random_quads, BruteStore, Rng};
use oxrdf::{Literal, NamedNode, TermRef};

fn lit(lex: &str, dt: &str) -> oxrdf::Term {
    Literal::new_typed_literal(lex, NamedNode::new(format!("{XSD}{dt}")).unwrap()).into()
}

#[test]
fn inline_roundtrip_and_canonicality() {
    let inline = [
        ("0", "integer"), ("-5", "integer"), ("576460752303423487", "integer"), ("-576460752303423488", "integer"),
        ("true", "boolean"), ("false", "boolean"),
        ("0", "decimal"), ("-12", "decimal"), ("5.0", "decimal"), ("0.0", "decimal"), ("-0.5", "decimal"),
        ("12.25", "decimal"), ("0.000000000000001", "decimal"),
        ("2020-01-31T23:59:59Z", "dateTime"), ("1999-12-31T00:00:00.5+05:30", "dateTime"),
        ("0001-01-01T00:00:00", "dateTime"), ("2024-02-29T12:00:00.125-14:00", "dateTime"),
        ("2020-01-01T00:00:00+00:00", "dateTime"),
        ("2020-02-29", "date"), ("1970-01-01Z", "date"), ("0001-01-01-01:30", "date"),
        ("", "string"), ("abc", "string"), ("1234567", "string"), ("é€", "string"),
    ];
    for (lex, dt) in inline {
        let t = lit(lex, dt);
        let id = encode_inline(t.as_ref()).unwrap_or_else(|| panic!("{lex}^^{dt} should inline"));
        assert!(id.is_inline(), "{lex}");
        let back = decode_inline(id).unwrap();
        assert_eq!(back.to_oxrdf(), t, "{lex}^^{dt} round trip");
        assert_eq!(id.kind(), Kind::Literal);
    }
    let not_inline = [
        ("01", "integer"), ("+1", "integer"), ("-0", "integer"), ("576460752303423488", "integer"), ("1.0", "integer"),
        ("1", "boolean"), ("TRUE", "boolean"),
        ("-0", "decimal"), ("-0.0", "decimal"), ("1.50", "decimal"), ("+1.5", "decimal"), (".5", "decimal"),
        ("5.", "decimal"), ("01.5", "decimal"), ("0.0000000000000001", "decimal"),
        ("2020-01-01T00:00:00.000Z", "dateTime"), ("2020-01-01T24:00:00Z", "dateTime"), ("2020-02-30T00:00:00", "dateTime"),
        ("2020-01-01T00:00:00-00:00", "dateTime"), ("2020-01-01T00:00:00.1234Z", "dateTime"),
        ("2020-01-01T00:00:00+14:30", "dateTime"), ("6001-01-01T00:00:00", "dateTime"), ("-0001-01-01T00:00:00", "dateTime"),
        ("2021-02-29", "date"), ("20201-01-01", "date"),
        ("12345678", "string"),
        ("1.0E0", "double"), ("x", "anyURI"),
    ];
    for (lex, dt) in not_inline {
        assert!(encode_inline(lit(lex, dt).as_ref()).is_none(), "{lex}^^{dt} must NOT inline");
    }
    // language-tagged literals never inline
    let l: oxrdf::Term = Literal::new_language_tagged_literal("ab", "en").unwrap().into();
    assert!(encode_inline(l.as_ref()).is_none());
}

#[test]
fn int_order_is_signed_monotonic() {
    let mut r = Rng(7);
    let mut vals: Vec<i64> = (0..1000).map(|_| (r.next_u64() as i64) >> 5).collect();
    vals.extend([TermId::INT_MIN, TermId::INT_MAX, -1, 0, 1]);
    vals.sort();
    let ids: Vec<TermId> = vals.iter().map(|v| TermId::from_int(*v).unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] <= w[1]));
    let (lo, hi) = TermId::int_raw_range(-3, 3);
    assert_eq!(TermId(lo).as_int(), Some(-3));
    assert_eq!(TermId(hi).as_int(), Some(3));
}

#[test]
fn tags_are_disjoint() {
    assert_eq!(TermId::temp(5, Tag::Lang).temp_index(), Some(5));
    assert_eq!(TermId::temp(5, Tag::Lang).temp_tag(), Tag::Lang);
    assert_eq!(TermId::temp(9, Tag::BNode).kind(), Kind::BNode);
    assert!(TermId::temp(9, Tag::BNode).is_bnode());
    assert!(!TermId::temp(1, Tag::Iri).is_storable());
    assert!(!TermId::ERROR.is_storable());
    assert!(TermId::bnode(3).is_storable() && !TermId::bnode(3).is_dict());
    assert_eq!(XSD_INTEGER.dict_row(), 2);
    assert_eq!(parse_bnode_label(&bnode_label(TermId::bnode(0x1f))), Some(TermId::bnode(0x1f)));
    assert_eq!(parse_bnode_label("b01"), None);
}

#[test]
fn golden_term_hashes() {
    let h1 = term_hash(Tag::Iri, "http://example.org/a", "");
    let h2 = term_hash(Tag::Str, "a", "");
    let h3 = term_hash(Tag::Lang, "a", "en");
    let h4 = term_hash(Tag::Typed, "1.0E0", "http://www.w3.org/2001/XMLSchema#double");
    println!("GOLDEN {h1} {h2} {h3} {h4}");
    assert_eq!([h1, h2, h3, h4], GOLDEN);
    let k = TermKey::of(TermRef::from(NamedNode::new("http://example.org/a").unwrap().as_ref())).unwrap();
    assert_eq!(k.hash64(), h1);
}
const GOLDEN: [i64; 4] = [-5228397991068580721, 9093457548028347866, 227138330714864330, 614094328781894807];

#[test]
fn candidates_cover_every_shape() {
    for mask in 0..16u8 {
        let bound = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0];
        let c = IndexOrder::candidates(bound);
        assert!(!c.is_empty(), "no order for {bound:?}");
    }
    assert!(IndexOrder::candidates([true, false, true, false]).iter().any(|(o, _)| *o == IndexOrder::Gpso));
}

#[test]
fn conformance_suite_self_test() {
    let quads = random_quads(42, 400, true);
    let store = BruteStore { quads: std::sync::Arc::new(quads.clone()) };
    cursor_conformance(&store, &quads, 9, 6);
    assert!(!store.named_graphs().unwrap().is_empty());
}
