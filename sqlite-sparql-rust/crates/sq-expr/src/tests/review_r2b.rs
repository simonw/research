//! Adversarial review tests, round 2b (expr lane: ValueCache admission filter, tag-decided
//! value classes, regex affix pre-filter). Each `review_r2b_*` test pins one behaviour; a
//! failing one documents a defect.
//!
//! The regex affix pre-filter (`regex::plan_for` / `trailing_literal`) reads the literal run
//! before a final `$` from the raw pattern text, but the pattern is handed to the `regex`
//! crate almost verbatim by `translate`, which accepts two pieces of `regex`-crate syntax that
//! XPath does not have: inline flag groups (`(?i)`, `(?m)`, `(?x)`, ...) and one-letter
//! Unicode classes (`\pL`, `\PN`). Both change what the characters before `$` mean, so the
//! pre-filter rejects texts that the engine's own automaton matches (and that matched before
//! round 2, when only `^literal` prefixes were pre-filtered). The automaton is what
//! `regex::compile` builds, so the expected answers below are the automaton's.
use sq_core::TermId;

use super::harness::Harness;

/// The `regex` crate automaton for a pattern that `translate` passes through unchanged (no
/// `.`, `\s`, `\w`, classes with `&`/`~`), with the XPath `i` flag mapped as `compile` maps it.
fn automaton(pattern: &str, i: bool) -> regex::Regex {
    regex::RegexBuilder::new(pattern).case_insensitive(i).build().expect("valid pattern")
}

/// REGEX(?t, pattern[, flags]) through the full expression pipeline, plus the automaton's
/// answer for the same text.
fn regex_ebv(h: &mut Harness, text: &str, pattern: &str, flags: &str) -> (Option<bool>, bool) {
    let lit = format!("{text:?}");
    let id: TermId = h.term(&lit);
    let pat = format!("{pattern:?}");
    let expr = if flags.is_empty() {
        format!("REGEX(?t, {pat})")
    } else {
        format!("REGEX(?t, {pat}, {flags:?})")
    };
    let got = h.ebv(&expr, &[("t", id)]);
    (got, automaton(pattern, flags.contains('i')).is_match(text))
}

/// `(?i)` before the literal run that ends the pattern makes that run case-insensitive, but
/// the pre-filter compares it byte-exactly (`Plan::Affix { ci: false }`): "xyzABC" is rejected
/// although the automaton (and the engine before round 2, where an unanchored pattern always
/// ran the automaton) matches it. `(?i)` is the usual way to ask for case-insensitivity in
/// Java/PCRE regexes, so SPARQL users coming from Jena write it.
#[test]
fn review_r2b_inline_i_flag_before_the_suffix_is_honoured() {
    let mut h = Harness::new();
    for (text, pattern) in [
        ("xyzABC", "(?i)abc$"),
        ("Person 17", "(?i)person 1[0-9]*7$"),
        ("abCD", "^ab(?i)cd$"),
        ("ABC", "(?i)^abc$"),
    ] {
        let (got, want) = regex_ebv(&mut h, text, pattern, "");
        assert!(want, "the automaton matches {text:?} with {pattern:?}");
        assert_eq!(got, Some(want), "REGEX({text:?}, {pattern:?})");
    }
}

/// `(?m)` turns the final `$` into an end-of-line anchor, so "abc\nxyz" matches `(?m)abc$`;
/// the pre-filter still requires the text to end with "abc".
#[test]
fn review_r2b_inline_m_flag_moves_the_end_anchor() {
    let mut h = Harness::new();
    for (text, pattern) in [("abc\nxyz", "(?m)abc$"), ("abc\nx", "^(?m)abc$")] {
        let (got, want) = regex_ebv(&mut h, text, pattern, "");
        assert!(want, "the automaton matches {text:?} with {pattern:?}");
        assert_eq!(got, Some(want), "REGEX({text:?}, {pattern:?})");
    }
}

/// Under `(?x)` spaces are ignored and `#` starts a comment that swallows the `$`: the
/// pre-filter requires the literal suffixes "a b c" / "abc#c", the automaton does not.
#[test]
fn review_r2b_inline_x_flag_spaces_and_comments_are_honoured() {
    let mut h = Harness::new();
    for (text, pattern) in [("abc", "(?x)a b c$"), ("zabcz", "(?x)abc#c$")] {
        let (got, want) = regex_ebv(&mut h, text, pattern, "");
        assert!(want, "the automaton matches {text:?} with {pattern:?}");
        assert_eq!(got, Some(want), "REGEX({text:?}, {pattern:?})");
    }
}

/// `translate` passes `\p` / `\P` through, and the `regex` crate reads `\pL` as the one-letter
/// class L. `trailing_literal` assumes an escape is one character long, so it takes "Lx" as
/// the literal suffix of `\pLx$` and rejects "ax".
#[test]
fn review_r2b_one_letter_unicode_class_is_not_part_of_the_suffix() {
    let mut h = Harness::new();
    for (text, pattern) in [("ax", r"\pLx$"), ("1x", r"\PLx$"), ("a7", r"^\pL7$"), ("é", r"^\pL$")] {
        let (got, want) = regex_ebv(&mut h, text, pattern, "");
        assert!(want, "the automaton matches {text:?} with {pattern:?}");
        assert_eq!(got, Some(want), "REGEX({text:?}, {pattern:?})");
    }
}

/// Property check (passes): under the XPath `i` flag the ASCII affixes agree with the
/// automaton on texts whose characters fold to ASCII letters from outside ASCII (U+212A KELVIN
/// SIGN, U+017F LONG S), Turkish dotted / dotless i (U+0130, U+0131), the Greek sigmas, and
/// texts where such a character sits inside, before or after the affix region.
#[test]
fn review_r2b_case_insensitive_affixes_agree_with_the_automaton_on_case_orbits() {
    let patterns = [
        "^k[0-9]*s$",
        "^is[a-z]*$",
        "^ab[^x]*ki$",
        "[0-9]+sk$",
        "^ı[a-z]*$",
        "^i[a-z]*I$",
        "^σ[a-z]*ς$",
        "^xσ[a-z]*sx$",
        "^ki([a-z]|ſ)*$",
        "^person 1[0-9]*7$",
        "^@`[x]*\\{\\[$",
    ];
    let texts = [
        "ks", "KS", "\u{212A}s", "k\u{17F}", "\u{212A}0\u{17F}", "Is", "\u{130}s", "\u{131}s", "ısx", "İsx", "ISX",
        "iabI", "\u{130}ab\u{131}", "ıabI", "σabς", "Σabσ", "ςabΣ", "xΣqqSX", "xσqq\u{17F}x", "KI", "k\u{130}",
        "\u{212A}\u{131}", "PERSON 127", "per\u{17F}on 17", "PERSON 1\u{212A}7", "@`x{[", "`@x[{", "ab12\u{212A}I",
        "AB12KI", "ab12k\u{131}", "12SK", "12\u{17F}\u{212A}", "",
    ];
    let mut bad = vec![];
    for p in patterns {
        let r = crate::regex::compile(p, "i").expect("compiles");
        let raw = automaton(p, true);
        for t in texts {
            if r.is_match(t) != raw.is_match(t) {
                bad.push(format!("{p:?} /i on {t:?}: plan {} automaton {}", r.is_match(t), raw.is_match(t)));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
