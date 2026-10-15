//! REGEX / REPLACE support (§6.6): XPath regular expressions translated to the `regex` crate.
//!
//! * Flags: `i` case-insensitive, `s` dot-all, `m` multi-line, `x` remove whitespace outside
//!   character classes (done here, so `#` keeps its literal meaning), `q` literal pattern.
//!   Any other flag is an error.
//! * XPath-only constructs are errors: `\i \I \c \C`, back-references and character class
//!   subtraction (`[a-z-[aeiou]]`). XPath's `\s` and `\w` and its non-dot-all `.` (which also
//!   excludes `\r`) are translated to their exact XPath definitions.
//! * Constant patterns are compiled once; dynamic (pattern, flags) pairs are cached in the
//!   execution's `ValueCache`. Anchored literal prefixes and literal patterns (a single
//!   top-level branch only) are checked with plain string operations before (or instead of)
//!   running the automaton.
//! * XSD character classes have no set operators: `&` and `~` are escaped so the `regex`
//!   crate never reads `&&` / `~~` as intersection / symmetric difference; `--` is an error.
//! * REPLACE replacement strings are prepared once per (regex, replacement) pair when both
//!   are constant.
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
    /// The pattern matches the empty string: REPLACE with it is an error (FORX0003).
    matches_empty: bool,
}

/// A REPLACE replacement string prepared for one compiled regex: `$N` references translated
/// to `${N}` (capped by the regex's group count), or kept literal under the `q` flag.
#[derive(Debug)]
pub struct Replacement {
    text: Box<str>,
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

    /// Prepares a replacement string for [`CompiledRegex::replace`]. None = invalid
    /// replacement (FORX0004).
    pub fn prepare_replacement(&self, replacement: &str) -> Option<Replacement> {
        if self.literal {
            return Some(Replacement { text: replacement.into(), literal: true });
        }
        let text = translate_replacement(replacement, self.re.captures_len() - 1)?;
        Some(Replacement { text: text.into(), literal: false })
    }

    /// fn:replace with a replacement prepared by this regex. None = error (the pattern matches
    /// the empty string).
    pub fn replace(&self, text: &str, replacement: &Replacement) -> Option<String> {
        if self.matches_empty {
            return None;
        }
        let out = if replacement.literal {
            self.re.replace_all(text, regex::NoExpand(&replacement.text))
        } else {
            self.re.replace_all(text, &*replacement.text)
        };
        Some(out.into_owned())
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
                        // Class subtraction `-[…]` is unsupported; `--` is not XSD syntax
                        // (and would be set difference for the `regex` crate).
                        '-' if matches!(chars.peek(), Some('[' | '-')) => return None,
                        '[' => out.push_str(r"\["),
                        // XSD classes have no set operators: `&&` / `~~` would be
                        // intersection / symmetric difference for the `regex` crate.
                        '&' => out.push_str(r"\&"),
                        '~' => out.push_str(r"\~"),
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

/// True if `pattern` has a `|` outside every group and character class: `^abc|xyz` is
/// `(^abc)|(xyz)`, so neither branch's literal prefix is required. Classes are skipped the
/// way `translate` reads them (optional `^`, a leading `]` is literal, `\` escapes).
fn has_top_level_alternation(pattern: &str) -> bool {
    let mut depth = 0usize;
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '[' => {
                chars.next_if_eq(&'^');
                chars.next_if_eq(&']');
                loop {
                    match chars.next() {
                        // Unterminated class: the pattern does not compile anyway.
                        None => return false,
                        Some(']') => break,
                        Some('\\') => {
                            chars.next();
                        }
                        Some(_) => {}
                    }
                }
            }
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// String-level plan for patterns that are (anchored) literals. Only a single top-level
/// branch can take a shortcut.
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
    if f.m || has_top_level_alternation(pattern) {
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
    let matches_empty = re.is_match("");
    Some(CompiledRegex { re, plan: plan_for(pattern, f), literal: f.q, matches_empty })
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

/// Per-execution cache of dynamically built regexes (`None` = invalid regex), keyed
/// pattern → flags so that a lookup borrows the row's strings (no allocation on a hit).
#[derive(Default)]
pub struct RegexCache {
    map: FxHashMap<Box<str>, FxHashMap<Box<str>, Option<Arc<CompiledRegex>>>>,
    len: usize,
}

impl RegexCache {
    pub fn get(&mut self, pattern: &str, flags: &str) -> Option<Arc<CompiledRegex>> {
        if let Some(r) = self.map.get(pattern).and_then(|by_flags| by_flags.get(flags)) {
            return r.clone();
        }
        if self.len >= CACHE_LIMIT {
            self.map.clear();
            self.len = 0;
        }
        let r = compile(pattern, flags).map(Arc::new);
        self.map.entry(pattern.into()).or_default().insert(flags.into(), r.clone());
        self.len += 1;
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
            // A top-level alternation means no branch's prefix is required.
            ("^abc|xyz", "fooxyz"),
            ("^abc|xyz", "abcd"),
            ("^abc|xyz", "foo"),
            ("^abc$|xyz", "xyz"),
            ("^ab|z", "zzz"),
            ("^abc|", "x"),
            ("^z|c$", "abc"),
            ("^a|^b", "b"),
            ("^ab(c|d)", "abd"),
            ("^ab(c|d)", "xabd"),
            ("^a[|]b", "a|b"),
            ("^a[|]b", "b"),
            (r"^a\|b", "a|b"),
            (r"^a\|b", "b"),
            ("^a[]|]b", "b"),
        ];
        for flags in ["", "s"] {
            for (p, t) in cases {
                let r = compile(p, flags).unwrap();
                let f = parse_flags(flags).unwrap();
                let raw = RegexBuilder::new(&translate(p, f).unwrap()).dot_matches_new_line(f.s).build().unwrap();
                assert_eq!(r.is_match(t), raw.is_match(t), "{p} /{flags} on {t} (plan {:?})", r.plan);
            }
        }
        assert!(matches!(compile("^abc", "").unwrap().plan, Plan::StartsWith(_)));
        assert!(matches!(compile("^abc$", "").unwrap().plan, Plan::Equals(_)));
        assert!(matches!(compile("^abc.*d", "").unwrap().plan, Plan::Prefix(_)));
        assert!(matches!(compile("^ab(c|d)", "").unwrap().plan, Plan::Prefix(_)));
        assert!(matches!(compile("abc", "").unwrap().plan, Plan::Contains(_)));
        assert!(matches!(compile("abc", "i").unwrap().plan, Plan::Regex));
        assert!(matches!(compile("^abc|xyz", "").unwrap().plan, Plan::Regex));
        assert!(matches!(compile("^abc$|x", "").unwrap().plan, Plan::Regex));
        assert!(matches!(compile("^a[|]b", "").unwrap().plan, Plan::Prefix(_)));
    }

    #[test]
    fn top_level_alternation() {
        assert!(has_top_level_alternation("a|b"));
        assert!(has_top_level_alternation("^abc|"));
        assert!(has_top_level_alternation("(a)|b"));
        assert!(has_top_level_alternation("[]]|b"));
        assert!(!has_top_level_alternation("(a|b)"));
        assert!(!has_top_level_alternation("^ab(c|(d|e))f"));
        assert!(!has_top_level_alternation(r"a\|b"));
        assert!(!has_top_level_alternation("[|]"));
        assert!(!has_top_level_alternation(r"[\]|]"));
        assert!(!has_top_level_alternation("[^]|]"));
    }

    #[test]
    fn class_set_operators_are_literal() {
        assert_eq!(m("^[a&&b]$", "", "&"), Some(true));
        assert_eq!(m("^[a&&b]$", "", "a"), Some(true));
        assert_eq!(m("^[a&&b]$", "", "b"), Some(true));
        assert_eq!(m("^[a~~b]$", "", "~"), Some(true));
        assert_eq!(m("^[&]$", "", "&"), Some(true));
        assert_eq!(m("a&&b", "", "xa&&by"), Some(true));
        assert_eq!(m("^[a-c]$", "", "b"), Some(true));
        assert_eq!(m("^[a-]$", "", "-"), Some(true));
        assert!(compile("[a--b]", "").is_none());
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

    fn replace(p: &str, f: &str, text: &str, repl: &str) -> Option<String> {
        let r = compile(p, f)?;
        r.replace(text, &r.prepare_replacement(repl)?)
    }

    #[test]
    fn replace_semantics() {
        assert_eq!(replace("(a)(b)?", "", "xaby a", "[$1$2]").as_deref(), Some("x[ab]y [a]"));
        assert_eq!(replace("a*", "", "baaac", "x"), None);
        assert_eq!(replace("o", "q", "foo", "$0").as_deref(), Some("f$0$0"));
        assert_eq!(replace("b", "i", "aBc", "Z").as_deref(), Some("aZc"));
        assert_eq!(replace("(b)", "", "abc", "$1$"), None);
        // One prepared replacement serves every row.
        let r = compile("(o)", "").unwrap();
        let prepared = r.prepare_replacement("[$1]").unwrap();
        assert_eq!(r.replace("foo", &prepared).as_deref(), Some("f[o][o]"));
        assert_eq!(r.replace("bar", &prepared).as_deref(), Some("bar"));
    }

    #[test]
    fn cache_reuses_and_remembers_errors() {
        let mut c = RegexCache::default();
        let a = c.get("a+", "").unwrap();
        let b = c.get("a+", "").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        let ai = c.get("a+", "i").unwrap();
        assert!(!Arc::ptr_eq(&a, &ai));
        assert!(Arc::ptr_eq(&ai, &c.get("a+", "i").unwrap()));
        assert!(c.get("(", "").is_none());
        assert!(c.get("a", "z").is_none());
        assert_eq!(c.len, 4);
        for i in 0..CACHE_LIMIT {
            c.get(&format!("x{i}"), "");
        }
        assert!(c.len <= CACHE_LIMIT);
    }
}
