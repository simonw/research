//! Per-row SPARQL-JSON (the `json` column of `sparql_rows`): one bindings object, encoded
//! exactly like an element of `results.bindings` in sparesults' JSON writer.
use std::fmt::Write as _;
use std::sync::Arc;

use sq_core::codec::XSD_STRING_IRI;
use sq_core::term::Term;

/// SPARQL-JSON bindings object for one row, e.g.
/// `{"s":{"type":"uri","value":"http://ex/a"},"n":{"type":"literal","value":"1","datatype":"…"}}`.
/// Unbound variables are omitted; `vars` and `row` are parallel.
pub fn solution_json(vars: &[String], row: &[Option<Arc<Term>>]) -> String {
    let mut out = String::with_capacity(16 + 48 * vars.len());
    append_solution_json(&mut out, vars, row);
    out
}

/// [`solution_json`] appending to a reusable buffer.
pub fn append_solution_json(out: &mut String, vars: &[String], row: &[Option<Arc<Term>>]) {
    debug_assert_eq!(vars.len(), row.len(), "variables and row must be parallel");
    out.push('{');
    let mut first = true;
    for (var, term) in vars.iter().zip(row) {
        let Some(term) = term else { continue };
        if !first {
            out.push(',');
        }
        first = false;
        push_json_string(out, var);
        out.push(':');
        append_term_json(out, term);
    }
    out.push('}');
}

/// SPARQL-JSON encoding of one RDF term: `{"type":"uri"|"bnode"|"literal","value":…}` plus
/// `xml:lang` for language-tagged literals or `datatype` for typed non-xsd:string literals.
pub fn append_term_json(out: &mut String, term: &Term) {
    match term {
        Term::Iri(iri) => {
            out.push_str(r#"{"type":"uri","value":"#);
            push_json_string(out, iri);
        }
        Term::BNode(label) => {
            out.push_str(r#"{"type":"bnode","value":"#);
            push_json_string(out, label);
        }
        Term::Literal(l) => {
            out.push_str(r#"{"type":"literal","value":"#);
            push_json_string(out, &l.lex);
            if let Some(lang) = &l.lang {
                out.push_str(r#","xml:lang":"#);
                push_json_string(out, lang);
            } else if &*l.datatype != XSD_STRING_IRI {
                out.push_str(r#","datatype":"#);
                push_json_string(out, &l.datatype);
            }
        }
    }
    out.push('}');
}

/// JSON string literal with the same escaping as sparesults (json-event-parser): `"` and `\`
/// escaped, the C0 controls as `\b \f \n \r \t` or `\u00xx`, everything else verbatim.
fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    let mut plain_from = 0;
    for (i, c) in s.char_indices() {
        let escape = match c {
            '"' => "\\\"",
            '\\' => "\\\\",
            '\n' => "\\n",
            '\r' => "\\r",
            '\t' => "\\t",
            '\x08' => "\\b",
            '\x0C' => "\\f",
            c if c < ' ' => "",
            _ => continue,
        };
        out.push_str(&s[plain_from..i]);
        if escape.is_empty() {
            // Infallible: writing to a String.
            let _ = write!(out, "\\u{:04x}", c as u32);
        } else {
            out.push_str(escape);
        }
        plain_from = i + c.len_utf8();
    }
    out.push_str(&s[plain_from..]);
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::term::Literal;

    fn arc(t: Term) -> Option<Arc<Term>> {
        Some(Arc::new(t))
    }

    #[test]
    fn encodes_every_term_kind() {
        let vars: Vec<String> = ["i", "b", "s", "l", "t", "u"].iter().map(|v| v.to_string()).collect();
        let row = vec![
            arc(Term::Iri("http://ex/a".into())),
            arc(Term::BNode("b1".into())),
            arc(Term::Literal(Literal::simple("x"))),
            arc(Term::Literal(Literal::lang("chat", "fr"))),
            arc(Term::Literal(Literal::typed("1", "http://www.w3.org/2001/XMLSchema#integer"))),
            None,
        ];
        assert_eq!(
            solution_json(&vars, &row),
            concat!(
                r#"{"i":{"type":"uri","value":"http://ex/a"},"#,
                r#""b":{"type":"bnode","value":"b1"},"#,
                r#""s":{"type":"literal","value":"x"},"#,
                r#""l":{"type":"literal","value":"chat","xml:lang":"fr"},"#,
                r#""t":{"type":"literal","value":"1","datatype":"http://www.w3.org/2001/XMLSchema#integer"}}"#
            )
        );
    }

    #[test]
    fn empty_and_all_unbound_rows() {
        assert_eq!(solution_json(&[], &[]), "{}");
        assert_eq!(solution_json(&["x".to_string()], &[None]), "{}");
    }

    #[test]
    fn escapes_like_sparesults() {
        let mut out = String::new();
        push_json_string(&mut out, "a\"b\\c\nd\re\tf\x08g\x0Ch\x01i\x1fé€😀/");
        assert_eq!(out, r#""a\"b\\c\nd\re\tf\bg\fh\u0001i\u001fé€😀/""#);
        // The output is valid JSON that decodes back to the input.
        let decoded: String = serde_json::from_str(&out).unwrap();
        assert_eq!(decoded, "a\"b\\c\nd\re\tf\x08g\x0Ch\x01i\x1fé€😀/");
    }

    #[test]
    fn appends_to_existing_buffer() {
        let mut out = String::from("[");
        append_solution_json(&mut out, &["v".to_string()], &[arc(Term::Iri("http://ex/v".into()))]);
        out.push(']');
        assert_eq!(out, r#"[{"v":{"type":"uri","value":"http://ex/v"}}]"#);
    }
}
