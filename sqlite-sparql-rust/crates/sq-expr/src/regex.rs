//! REGEX / REPLACE support (§6.6): XPath regular expressions translated to the `regex` crate.
//!
//! * Flags: `i` case-insensitive, `s` dot-all, `m` multi-line, `x` remove whitespace outside
//!   character classes (done here, so `#` keeps its literal meaning), `q` literal pattern.
//!   Any other flag is an error.
//! * XPath-only constructs are errors: `\i \I \c \C`, back-references and character class
//!   subtraction (`[a-z-[aeiou]]`). XPath's `\s` and `\w` and its non-dot-all `.` (which also
//!   excludes `\r`) are translated to their exact XPath definitions.
//! * Constant patterns are compiled once; dynamic (pattern, flags) pairs are cached in the
//!   execution's `ValueCache`. Anchored literal prefixes and literal patterns are checked with
//!   plain string operations before (or instead of) running the automaton.
use std::sync::Arc;

use regex::{Regex, RegexBuilder};
use rustc_hash::FxHashMap;

/// Compiled-program size limit (protects against pathological patterns).
const SIZE_LIMIT: usize = 1 << 20;
/// Dynamic patterns kept per execution before the cache is reset.
const CACHE_LIMIT: usize = 256;

#[derive(Debug)]
enum Plan {
    /// Run the automaton.
    Regex,
    /// The pattern is a literal string (no anchors): substring test.
    Contains(Box<str>),
    /// `^literal`: prefix test.
    StartsWith(Box<str>),
    /// `^literal$`: equality test.
    Equals(Box<str>),
    /// `^literal…`: a match requires the prefix, then the automaton decides.
    Prefix(Box<str>),
}

/// A compiled XPath regular expression.
#[derive(Debug)]
pub struct CompiledRegex {
    re: Regex,
    plan: Plan,
    /// `q` flag: REPLACE's replacement string is literal too.
    literal: bool,
}

impl CompiledRegex {
    /// fn:matches.
    pub fn is_match(&self, text: &str) -> bool {
        match &self.plan {
            Plan::Regex => self.re.is_match(text),
            Plan::Contains(s) => text.contains(&**s),
            Plan::StartsWith(s) => text.starts_with(&**s),
            Plan::Equals(s) => text == &**s,
            Plan::Prefix(s) => text.starts_with(&**s) && self.re.is_match(text),
        }
    }

    /// fn:replace. None = error (the pattern matches the empty string, or the replacement
    /// string is invalid).
    pub fn replace(&self, text: &str, replacement: &str) -> Option<String> {
        if self.re.is_match("") {
            return None;
        }
        if self.literal {
            return Some(self.re.replace_all(text, regex::NoExpand(replacement)).into_owned());
        }
        let repl = translate_replacement(replacement, self.re.captures_len() - 1)?;
        Some(self.re.replace_all(text, repl.as_str()).into_owned())
    }
}

#[derive(Default, Copy, Clone)]
struct Flags {
    i: bool,
    s: bool,
    m: bool,
    x: bool,
    q: bool,
}

fn parse_flags(flags: &str) -> Option<Flags> {
    let mut f = Flags::default();
    for c in flags.chars() {
        match c {
            'i' => f.i = true,
            's' => f.s = true,
            'm' => f.m = true,
            'x' => f.x = true,
            'q' => f.q = true,
            _ => return None,
        }
    }
    Some(f)
}

const XPATH_S: &str = r"\t\n\r ";
const XPATH_W_NOT: &str = r"\p{P}\p{Z}\p{C}";

/// Translates an XPath regex into `regex` crate syntax. None = invalid or unsupported.
fn translate(pattern: &str, f: Flags) -> Option<String> {
    let mut out = String::with_capacity(pattern.len() + 8);
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => translate_escape(chars.next()?, &mut out, false)?,
            '[' => {
                // Character class: copied with escapes translated; subtraction is unsupported.
                out.push('[');
                if chars.peek() == Some(&'^') {
                    chars.next();
                    out.push('^');
                }
                if chars.peek() == Some(&']') {
                    // A leading ']' is literal (lenient: XSD requires `\]`).
                    chars.next();
                    out.push_str(r"\]");
                }
                loop {
                    match chars.next()? {
                        ']' => break,
                        '\\' => translate_escape(chars.next()?, &mut out, true)?,
                        '-' if chars.peek() == Some(&'[') => return None,
                        '[' => out.push_str(r"\["),
                        c => out.push(c),
                    }
                }
                out.push(']');
            }
            '.' if !f.s => out.push_str(r"[^\n\r]"),
            c if f.x && matches!(c, ' ' | '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    Some(out)
}

/// Translates the character after a backslash.
fn translate_escape(c: char, out: &mut String, in_class: bool) -> Option<()> {
    match c {
        // XML name-character escapes have no `regex` equivalent; digits are back-references.
        'i' | 'I' | 'c' | 'C' | '0'..='9' => return None,
        's' if in_class => out.push_str(XPATH_S),
        's' => {
            out.push('[');
            out.push_str(XPATH_S);
            out.push(']');
        }
        'S' => {
            out.push_str("[^");
            out.push_str(XPATH_S);
            out.push(']');
        }
        'w' => {
            out.push_str("[^");
            out.push_str(XPATH_W_NOT);
            out.push(']');
        }
        'W' if in_class => out.push_str(XPATH_W_NOT),
        'W' => {
            out.push('[');
            out.push_str(XPATH_W_NOT);
            out.push(']');
        }
        'p' | 'P' | 'd' | 'D' | 'n' | 'r' | 't' => {
            out.push('\\');
            out.push(c);
        }
        // Single-character escapes: \\ \| \. \- \^ \? \* \+ \{ \} \( \) \[ \] \$
        '\\' | '|' | '.' | '-' | '^' | '?' | '*' | '+' | '{' | '}' | '(' | ')' | '[' | ']' | '$' => {
            out.push('\\');
            out.push(c);
        }
        _ => return None,
    }
    Some(())
}

const META: &[char] = &['\\', '.', '+', '*', '?', '(', ')', '|', '[', ']', '{', '}', '^', '$'];

/// String-level plan for patterns that are (anchored) literals.
fn plan_for(pattern: &str, f: Flags) -> Plan {
    if f.i || f.x {
        return Plan::Regex;
    }
    if f.q {
        return Plan::Contains(pattern.into());
    }
    if !pattern.contains(META) {
        return Plan::Contains(pattern.into());
    }
    if f.m {
        return Plan::Regex;
    }
    let Some(rest) = pattern.strip_prefix('^') else { return Plan::Regex };
    let end = rest.find(META).unwrap_or(rest.len());
    let (lit, tail) = rest.split_at(end);
    if tail.is_empty() {
        return Plan::StartsWith(lit.into());
    }
    if tail == "$" {
        return Plan::Equals(lit.into());
    }
    // A quantifier applies to the last literal character, which is then not required.
    let lit = if tail.starts_with(['*', '?', '{', '+']) {
        let mut cs = lit.chars();
        cs.next_back();
        cs.as_str()
    } else {
        lit
    };
    if lit.is_empty() { Plan::Regex } else { Plan::Prefix(lit.into()) }
}

/// Compiles an XPath regex with XPath flags. None = invalid pattern or flags (SPARQL error).
pub fn compile(pattern: &str, flags: &str) -> Option<CompiledRegex> {
    let f = parse_flags(flags)?;
    let translated = if f.q { regex::escape(pattern) } else { translate(pattern, f)? };
    let re = RegexBuilder::new(&translated)
        .case_insensitive(f.i)
        .multi_line(f.m && !f.q)
        .dot_matches_new_line(f.s && !f.q)
        .size_limit(SIZE_LIMIT)
        .build()
        .ok()?;
    Some(CompiledRegex { re, plan: plan_for(pattern, f), literal: f.q })
}

/// Translates an XPath replacement string (`$N` group references, `\$` and `\\` escapes) into
/// `regex` crate syntax. None = invalid replacement (FORX0004).
pub fn translate_replacement(repl: &str, groups: usize) -> Option<String> {
    let mut out = String::with_capacity(repl.len() + 4);
    let mut chars = repl.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                '$' => out.push_str("$$"),
                '\\' => out.push('\\'),
                _ => return None,
            },
            '$' => {
                let mut digits = String::new();
                while let Some(d) = chars.peek().copied().filter(char::is_ascii_digit) {
                    digits.push(d);
                    chars.next();
                }
                if digits.is_empty() {
                    return None;
                }
                // Strip trailing digits (kept as literals) until N ≤ max(groups, 9).
                let mut literal_tail = String::new();
                loop {
                    let n: usize = digits.parse().unwrap_or(usize::MAX);
                    if n <= groups {
                        out.push_str("${");
                        out.push_str(&n.to_string());
                        out.push('}');
                        break;
                    }
                    if n <= 9 {
                        break; // S < N ≤ 9: the zero-length string
                    }
                    literal_tail.insert(0, digits.pop().expect("non-empty"));
                }
                for d in literal_tail.chars() {
                    out.push(d);
                }
            }
            c => out.push(c),
        }
    }
    Some(out)
}

/// (pattern, flags).
type RegexKey = (Box<str>, Box<str>);

/// Per-execution cache of dynamically built regexes (`None` = invalid regex).
#[derive(Default)]
pub struct RegexCache {
    map: FxHashMap<RegexKey, Option<Arc<CompiledRegex>>>,
}

impl RegexCache {
    pub fn get(&mut self, pattern: &str, flags: &str) -> Option<Arc<CompiledRegex>> {
        if let Some(r) = self.map.get(&(pattern.into(), flags.into())) {
            return r.clone();
        }
        if self.map.len() >= CACHE_LIMIT {
            self.map.clear();
        }
        let r = compile(pattern, flags).map(Arc::new);
        self.map.insert((pattern.into(), flags.into()), r.clone());
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &str, f: &str, t: &str) -> Option<bool> {
        compile(p, f).map(|r| r.is_match(t))
    }

    #[test]
    fn flags() {
        assert_eq!(m("abc", "", "xxabcxx"), Some(true));
        assert_eq!(m("ABC", "i", "xxabcxx"), Some(true));
        assert_eq!(m("ABC", "", "xxabcxx"), Some(false));
        assert_eq!(m("a.c", "", "a\nc"), Some(false));
        assert_eq!(m("a.c", "s", "a\nc"), Some(true));
        assert_eq!(m("a.c", "", "a\rc"), Some(false));
        assert_eq!(m("^b$", "", "a\nb"), Some(false));
        assert_eq!(m("^b$", "m", "a\nb"), Some(true));
        assert_eq!(m("a b c", "x", "abc"), Some(true));
        assert_eq!(m("[ ]", "x", "a b"), Some(true));
        assert_eq!(m("a#b", "x", "a#b"), Some(true));
        assert_eq!(m("a.b", "q", "a.b"), Some(true));
        assert_eq!(m("a.b", "q", "axb"), Some(false));
        assert_eq!(m("A.B", "qi", "xa.bx"), Some(true));
        assert_eq!(m("abc", "g", "abc"), None);
    }

    #[test]
    fn xpath_only_constructs_are_errors() {
        assert!(compile(r"\i\c*", "").is_none());
        assert!(compile(r"[a-z-[aeiou]]", "").is_none());
        assert!(compile(r"(a)\1", "").is_none());
        assert!(compile(r"(", "").is_none());
        assert!(compile(r"\p{IsBasicLatin}", "").is_none());
        assert!(compile(r"\q", "").is_none());
    }

    #[test]
    fn xpath_classes() {
        assert_eq!(m(r"^\s$", "", "\u{a0}"), Some(false));
        assert_eq!(m(r"^\s$", "", "\t"), Some(true));
        assert_eq!(m(r"^[\s]$", "", " "), Some(true));
        assert_eq!(m(r"^\w+$", "", "abc_1é"), Some(false)); // '_' is punctuation in XPath
        assert_eq!(m(r"^\w+$", "", "abc1é"), Some(true));
        assert_eq!(m(r"^\d+$", "", "١٢"), Some(true));
        assert_eq!(m(r"^[^a]$", "", "b"), Some(true));
        assert_eq!(m(r"^[]a]$", "", "]"), Some(true));
        assert_eq!(m(r"^[a\-z]$", "", "-"), Some(true));
        assert_eq!(m(r"\p{Lu}", "", "aB"), Some(true));
    }

    #[test]
    fn literal_plans_agree_with_the_automaton() {
        let cases = [
            ("^abc", "abcdef"),
            ("^abc", "xabc"),
            ("^abc$", "abc"),
            ("^abc$", "abcd"),
            ("^ab*c", "ac"),
            ("^ab+c", "abbc"),
            ("^ab?", "a"),
            ("^ab.d", "abxd"),
            ("abc", "zabcz"),
            ("", "anything"),
            ("^", "x"),
            ("^(a|b)", "b"),
            ("^ab{2}", "abb"),
        ];
        for (p, t) in cases {
            let r = compile(p, "").unwrap();
            let raw = Regex::new(&translate(p, Flags::default()).unwrap()).unwrap();
            assert_eq!(r.is_match(t), raw.is_match(t), "{p} on {t} (plan {:?})", r.plan);
        }
        assert!(matches!(compile("^abc", "").unwrap().plan, Plan::StartsWith(_)));
        assert!(matches!(compile("^abc$", "").unwrap().plan, Plan::Equals(_)));
        assert!(matches!(compile("^abc.*d", "").unwrap().plan, Plan::Prefix(_)));
        assert!(matches!(compile("abc", "").unwrap().plan, Plan::Contains(_)));
        assert!(matches!(compile("abc", "i").unwrap().plan, Plan::Regex));
    }

    #[test]
    fn replacement_strings() {
        assert_eq!(translate_replacement("x$1y", 1).as_deref(), Some("x${1}y"));
        assert_eq!(translate_replacement("$0", 0).as_deref(), Some("${0}"));
        assert_eq!(translate_replacement(r"\$1", 1).as_deref(), Some("$$1"));
        assert_eq!(translate_replacement(r"a\\b", 0).as_deref(), Some(r"a\b"));
        assert_eq!(translate_replacement("$12", 1).as_deref(), Some("${1}2"));
        assert_eq!(translate_replacement("$5", 1).as_deref(), Some(""));
        assert_eq!(translate_replacement("$", 1), None);
        assert_eq!(translate_replacement(r"\n", 1), None);
    }

    #[test]
    fn replace_semantics() {
        let r = compile("(a)(b)?", "").unwrap();
        assert_eq!(r.replace("xaby a", "[$1$2]").as_deref(), Some("x[ab]y [a]"));
        assert_eq!(compile("a*", "").unwrap().replace("baaac", "x"), None);
        assert_eq!(compile("o", "q").unwrap().replace("foo", "$0").as_deref(), Some("f$0$0"));
        assert_eq!(compile("b", "i").unwrap().replace("aBc", "Z").as_deref(), Some("aZc"));
    }

    #[test]
    fn cache_reuses_and_remembers_errors() {
        let mut c = RegexCache::default();
        let a = c.get("a+", "").unwrap();
        let b = c.get("a+", "").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(c.get("(", "").is_none());
        assert!(c.get("a", "z").is_none());
    }
}
