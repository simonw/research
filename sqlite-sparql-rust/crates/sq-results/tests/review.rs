//! Adversarial review tests for sq-results.
use std::sync::Arc;

use sq_core::eval_api::SolutionSource;
use sq_core::term::{Literal, Term};
use sq_core::{Result, Tag, TermId};
use sq_results::{write_solutions, ResultFormat};

/// One variable `?o`, one row bound to a single dictionary term.
struct OneLiteral {
    vars: Vec<String>,
    term: Arc<Term>,
    done: bool,
}

impl SolutionSource for OneLiteral {
    fn variables(&self) -> &[String] {
        &self.vars
    }
    fn next_rows(&mut self, out: &mut Vec<TermId>, _max_rows: usize) -> Result<usize> {
        if std::mem::replace(&mut self.done, true) {
            return Ok(0);
        }
        out.push(TermId::from_dict(1024, Tag::Str));
        Ok(1)
    }
    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        out.extend(ids.iter().map(|_| Some(self.term.clone())));
        Ok(())
    }
}

/// XML 1.0 `Char` production: #x9 | #xA | #xD | [#x20-#xD7FF] | [#xE000-#xFFFD] | [#x10000-#x10FFFF].
fn is_xml10_char(c: char) -> bool {
    matches!(c, '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
}

/// An RDF literal may contain any Unicode character, including C0 controls such as U+0001
/// (e.g. data loaded from N-Triples `"a\u0001b"`). XML 1.0 cannot carry U+0001 at all, not
/// even as a character reference, so `write_solutions(…, Xml, …)` must either refuse the
/// result or produce a well-formed document. Today it writes the raw control character
/// (sparesults' XML writer only escapes `& < > " '`), and strict XML parsers (expat, used by
/// Python's `xml.*`, libxml2) reject the whole document.
#[test]
fn review_xml_output_is_well_formed_or_refused_for_control_characters() {
    let mut src = OneLiteral {
        vars: vec!["o".to_owned()],
        term: Arc::new(Term::Literal(Literal::simple("a\u{1}b"))),
        done: false,
    };
    let mut buf = Vec::new();
    match write_solutions(&mut src, ResultFormat::Xml, &mut buf) {
        Err(_) => {} // refusing is acceptable
        Ok(()) => {
            let text = String::from_utf8(buf).unwrap();
            let bad: Vec<char> = text.chars().filter(|c| !is_xml10_char(*c)).collect();
            assert!(bad.is_empty(), "SPARQL XML output is not well-formed XML 1.0 (raw {bad:?}): {text:?}");
        }
    }
}
