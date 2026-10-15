//! Recursion budget of a request's text, checked before anything parses it.
//!
//! spargebra's parser is recursive descent (nested brackets, and the right-recursive `+ - *
//! /` chains, recurse once per level), it folds `|| && UNION OPTIONAL FILTER BIND` chains,
//! property paths and group elements into trees as deep as the chain is long, and lowering,
//! planning, execution and `Drop` all recurse over those trees. A request nested a few
//! thousand levels deep overflows the host thread's stack, and a stack overflow is not a
//! panic: `catch_unwind` cannot contain it, the host process dies. So every request text is
//! scanned once ([`check`]) and refused when its estimated recursion *cost* exceeds
//! [`LIMIT`].
//!
//! **Cost model.** The text is split into frames (`( [ {` … `) ] }`); a frame's cost is its
//! own weight, plus one weight per chain element in it, plus the cost of its most expensive
//! child frame. The weights approximate the stack (in KiB) that the most expensive pipeline
//! stage uses per level, measured for every construct on the native and baseline engines,
//! so [`LIMIT`] keeps the worst case near 1 MiB: safe on 8 MiB main threads and on the
//! 2 MiB threads many hosts spawn. Chains are counted per frame kind:
//! * expression frames (`FILTER( … )`, `BIND( … )`, projections, function arguments): the
//!   operators `|| && + - * / !`;
//! * pattern frames (`{ … }` groups, blank-node lists, collections and path groups): child
//!   groups and FILTER / BIND elements, and path operators `/ | ^ * + ! ?` (each path triple
//!   is an element too), which become join, left-join, filter and extend chains of the
//!   enclosing group;
//! * data frames (`INSERT DATA`, `DELETE DATA` and the templates of CONSTRUCT / DELETE /
//!   INSERT): nothing, spargebra keeps their quads flat.
//!
//! **Soundness.** Strings, comments and IRIs are skipped, exactly as spargebra tokenizes them
//! (including its fallback from an unterminated `'''` to `''` + `'`), so brackets inside them
//! cost nothing — but skipping must never hide structure the parser sees. The one lexical
//! ambiguity in spargebra's grammar is `<`: an IRI (`<` up to the first `>`, if the content
//! is a valid IRI) or, inside an expression right after an operand, the `<` / `<=` operator.
//! Where both readings are possible the scanner counts the span as an expression too, and
//! refuses the request when the two readings could tokenize the rest of the text differently
//! (the span contains `'`, `#` or `\`, or unbalanced brackets). So `<` may be read as an IRI
//! only outside expressions, and every expression parenthesis must be recognized: those of the
//! top level, of a SELECT / GROUP BY / ORDER BY / HAVING clause, and the one after `FILTER` or
//! `BIND` (with at most a function name in between). These keywords are matched anywhere in a
//! word (spargebra reads `1FILTER(` as `1` then `FILTER(`); the keywords that make a frame
//! cheaper (`WHERE`, and `DATA` / `INSERT` / `DELETE` / `CONSTRUCT` before a top-level `{`)
//! only as whole words. Every other doubt errs towards a higher cost.
use sq_core::{Error, Result};

/// Maximum recursion cost of one request (see the module docs).
pub(crate) const LIMIT: u32 = 1000;

/// One operator of an expression chain (`||` and `&&` count once).
const W_OP: u32 = 1;
/// One property-path operator in a pattern (a path triple joins its group's chain).
const W_PATH: u32 = 4;
/// One chain element of a group: a child group, `FILTER( … )` or `BIND( … )`.
const W_CHILD: u32 = 4;
/// Frame weights.
const W_PAREN: u32 = 5;
const W_BRACKET: u32 = 5;
const W_BRACE: u32 = 8;

/// Checks a SPARQL query or update text against [`LIMIT`].
pub(crate) fn check(text: &str) -> Result<()> {
    Scanner::new(text.as_bytes(), Mode::Pattern, true).run().map(|_| ())
}

/// Parses a query after [`check`].
pub(crate) fn parse_query(text: &str, base: Option<&str>) -> Result<spargebra::Query> {
    check(text)?;
    sq_plan::parse_query(text, base)
}

/// Parses an update after [`check`].
pub(crate) fn parse_update(text: &str, base: Option<&str>) -> Result<spargebra::Update> {
    check(text)?;
    sq_plan::parse_update(text, base)
}

fn too_deep(cost: u32) -> Error {
    Error::Unsupported(format!(
        "the request nests too deeply (nesting cost {cost} exceeds {LIMIT}): reduce bracket nesting, operator \
         chains or the number of OPTIONAL / UNION / FILTER / BIND elements in one group"
    ))
}

/// How the parser reads a frame's content.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Mode {
    /// Graph patterns, blank-node property lists, collections, path groups.
    Pattern,
    /// Quad data and templates (flat in spargebra).
    Data,
    /// Expressions.
    Expr,
}

#[derive(Debug)]
struct Frame {
    mode: Mode,
    /// Closing byte (0 for the outermost frame).
    close: u8,
    weight: u32,
    items: u32,
    max_child: u32,
    /// The outermost frame (every `(` there is an expression).
    root: bool,
    /// A SELECT / GROUP BY / ORDER BY / HAVING clause is open (its `(` are expressions).
    clause: bool,
}

impl Frame {
    fn new(mode: Mode, close: u8, weight: u32, root: bool) -> Frame {
        Frame { mode, close, weight, items: 0, max_child: 0, root, clause: false }
    }

    fn cost(&self) -> u32 {
        self.weight.saturating_add(self.items).saturating_add(self.max_child)
    }
}

/// Keywords that classify frames.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Keyword {
    /// `FILTER` / `BIND`: the next `(` (or the one after a function name) is an expression.
    Constraint,
    /// `SELECT`, `GROUP`, `ORDER`, `HAVING`: starts a clause whose `(` are expressions.
    Clause,
    Where,
    /// `DATA`, `INSERT`, `DELETE`, `CONSTRUCT`: a following top-level `{` is quad data or a
    /// template.
    Template,
    Other,
}

/// The previous significant token. Words are names, numbers and keywords.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Token {
    None,
    Word(Keyword),
    Var,
    Iri,
    Str,
    Punct(u8),
}

impl Token {
    /// Whether an operand may end with this token (then a following `<` may be the
    /// comparison operator). Errs towards `true`.
    fn may_end_operand(self) -> bool {
        let Token::Punct(c) = self else { return self != Token::None };
        !matches!(c, b'(' | b',' | b'{' | b'[' | b';' | b'=' | b'<' | b'!' | b'&' | b'|' | b'+' | b'*' | b'/' | b'^')
    }
}

/// Bytes of variable names (a superset of VARNAME: the parser fails on the extra ones).
fn is_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80
}

/// Bytes of prefixed names, numbers and keywords (plus `\` escapes, see [`Scanner::word`]).
fn is_word_byte(c: u8) -> bool {
    is_name_byte(c) || matches!(c, b':' | b'%' | b'.' | b'-')
}

/// Classifies a word: the keywords that only ever raise the cost anywhere in it, the others
/// as the whole word (see the module docs).
fn keyword(word: &[u8]) -> Keyword {
    let contains = |k: &str| word.windows(k.len()).any(|w| w.eq_ignore_ascii_case(k.as_bytes()));
    let is = |k: &str| word.eq_ignore_ascii_case(k.as_bytes());
    if contains("FILTER") || contains("BIND") {
        Keyword::Constraint
    } else if ["SELECT", "GROUP", "ORDER", "HAVING"].into_iter().any(contains) {
        Keyword::Clause
    } else if is("WHERE") {
        Keyword::Where
    } else if ["DATA", "INSERT", "DELETE", "CONSTRUCT"].into_iter().any(is) {
        Keyword::Template
    } else {
        Keyword::Other
    }
}

/// spargebra's IRIREF is `<` up to the first `>`, kept only if the content is a valid IRI.
/// Returns the position of that `>` when the span may be one: no byte that oxiri rejects
/// (controls, space, `<`, `"`, `{`, `}`, `|`, `^`, `` ` ``) — a superset of the IRIs
/// spargebra accepts (`\` is kept: `\u` escapes are unescaped before validation).
fn iri_end(b: &[u8], open: usize) -> Option<usize> {
    for (k, &c) in b.iter().enumerate().skip(open + 1) {
        match c {
            b'>' => return Some(k),
            0..=0x20 | b'<' | b'"' | b'{' | b'}' | b'|' | b'^' | b'`' => return None,
            _ => {}
        }
    }
    None
}

/// Whether `()` / `[]` in `span` nest properly (never closing what the span did not open).
fn balanced(span: &[u8]) -> bool {
    let mut open: Vec<u8> = Vec::new();
    for &c in span {
        match c {
            b'(' => open.push(b')'),
            b'[' => open.push(b']'),
            b')' | b']' if open.pop() != Some(c) => return false,
            _ => {}
        }
    }
    open.is_empty()
}

/// End of the string literal opening at `at` (exclusive), following spargebra's ordered
/// choice: `'''…'''` when it terminates, else `'…'` (so an unterminated `'''` reads as the
/// empty `''` followed by a new literal). `None`: no literal parses here (the parser fails).
fn string_end(b: &[u8], at: usize) -> Option<usize> {
    let q = b[at];
    if b.get(at + 1) == Some(&q) && b.get(at + 2) == Some(&q) {
        if let Some(end) = long_string_end(b, at + 3, q) {
            return Some(end);
        }
    }
    short_string_end(b, at + 1, q)
}

/// Length of a valid `ECHAR` / `UCHAR` escape at `at` (which holds `\`).
fn escape_len(b: &[u8], at: usize) -> Option<usize> {
    let hex = |n: usize| {
        let digits = b.get(at + 2..at + 2 + n)?;
        digits.iter().all(u8::is_ascii_hexdigit).then_some(n + 2)
    };
    match b.get(at + 1)? {
        b't' | b'b' | b'n' | b'r' | b'f' | b'"' | b'\'' | b'\\' => Some(2),
        b'u' => hex(4),
        b'U' => hex(8),
        _ => None,
    }
}

/// `STRING_LITERAL1` / `2`: no raw line break, escapes must be valid.
fn short_string_end(b: &[u8], mut p: usize, q: u8) -> Option<usize> {
    loop {
        match *b.get(p)? {
            c if c == q => return Some(p + 1),
            b'\\' => p += escape_len(b, p)?,
            b'\n' | b'\r' => return None,
            _ => p += 1,
        }
    }
}

/// `STRING_LITERAL_LONG1` / `2`: `((q q / q)? (char / ECHAR / UCHAR))* q q q`.
fn long_string_end(b: &[u8], mut p: usize, q: u8) -> Option<usize> {
    loop {
        let prefix = match (b.get(p), b.get(p + 1)) {
            (Some(&x), Some(&y)) if x == q && y == q => 2,
            (Some(&x), _) if x == q => 1,
            _ => 0,
        };
        let element = match b.get(p + prefix) {
            Some(&c) if c != q && c != b'\\' => Some(1),
            Some(b'\\') => escape_len(b, p + prefix),
            _ => None,
        };
        match element {
            Some(n) => p += prefix + n,
            None => {
                let closes = b.len() >= p + 3 && b[p..p + 3].iter().all(|&c| c == q);
                return closes.then_some(p + 3);
            }
        }
    }
}

struct Scanner<'a> {
    b: &'a [u8],
    stack: Vec<Frame>,
    /// Σ (weight + items) over the open frames: a lower bound of the final cost.
    path: u32,
    prev: Token,
    /// A `FILTER` / `BIND` keyword was read, followed by nothing but a possible function name:
    /// the next `(` is an expression.
    constraint: bool,
}

impl<'a> Scanner<'a> {
    fn new(b: &'a [u8], mode: Mode, root: bool) -> Scanner<'a> {
        Scanner { b, stack: vec![Frame::new(mode, 0, 0, root)], path: 0, prev: Token::None, constraint: false }
    }

    fn top(&mut self) -> &mut Frame {
        self.stack.last_mut().expect("the outermost frame is never popped")
    }

    fn token(&mut self, t: Token) {
        match t {
            Token::Word(Keyword::Constraint) => self.constraint = true,
            // A function name (prefixed name, builtin or IRI) may stand before the `(`.
            Token::Word(_) | Token::Iri => {}
            _ => self.constraint = false,
        }
        self.prev = t;
    }

    /// Adds `w` to the chain of the frame at `at`.
    fn add_at(&mut self, at: usize, w: u32) -> Result<()> {
        let frame = &mut self.stack[at];
        frame.items = frame.items.saturating_add(w);
        self.path = self.path.saturating_add(w);
        if self.path > LIMIT { Err(too_deep(self.path)) } else { Ok(()) }
    }

    fn add(&mut self, w: u32) -> Result<()> {
        self.add_at(self.stack.len() - 1, w)
    }

    fn child_cost(&mut self, cost: u32) -> Result<()> {
        let top = self.top();
        top.max_child = top.max_child.max(cost);
        let bound = self.path.saturating_add(cost);
        if bound > LIMIT { Err(too_deep(bound)) } else { Ok(()) }
    }

    fn push(&mut self, mode: Mode, close: u8, weight: u32) -> Result<()> {
        self.stack.push(Frame::new(mode, close, weight, false));
        self.path = self.path.saturating_add(weight);
        if self.path > LIMIT { Err(too_deep(self.path)) } else { Ok(()) }
    }

    fn pop(&mut self) -> Result<()> {
        let frame = self.stack.pop().expect("only inner frames are popped");
        self.path = self.path.saturating_sub(frame.weight.saturating_add(frame.items));
        self.child_cost(frame.cost())
    }

    /// Runs the scan; returns the request's cost.
    fn run(mut self) -> Result<u32> {
        let b = self.b;
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            match c {
                b' ' | b'\t' | b'\n' | b'\r' => i += 1,
                b'#' => {
                    while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                        i += 1;
                    }
                }
                b'\'' | b'"' => {
                    // A literal that does not parse makes the parse fail right here; reading
                    // on after the quote only costs more.
                    i = string_end(b, i).unwrap_or(i + 1);
                    self.token(Token::Str);
                }
                b'<' => i = self.angle(i)?,
                b'?' | b'$' if b.get(i + 1).is_some_and(|&n| is_name_byte(n)) => {
                    i += 1;
                    while i < b.len() && is_name_byte(b[i]) {
                        i += 1;
                    }
                    self.token(Token::Var);
                }
                b'(' => {
                    self.open_paren()?;
                    i += 1;
                }
                b'[' => {
                    let mode = if self.top().mode == Mode::Data { Mode::Data } else { Mode::Pattern };
                    self.push(mode, b']', W_BRACKET)?;
                    self.token(Token::Punct(c));
                    i += 1;
                }
                b'{' => {
                    self.open_brace()?;
                    i += 1;
                }
                b')' | b']' | b'}' => {
                    // A close that does not match makes the parse fail; it is ignored.
                    if self.stack.len() > 1 && self.top().close == c {
                        self.pop()?;
                    }
                    self.token(Token::Punct(c));
                    i += 1;
                }
                b'|' | b'&' | b'+' | b'*' | b'/' | b'!' | b'^' | b'?' => {
                    let pair = b.get(i + 1) == Some(&c) && matches!(c, b'|' | b'&' | b'^');
                    if !(c == b'^' && pair) {
                        self.operator(c)?;
                    }
                    self.token(Token::Punct(c));
                    i += if pair { 2 } else { 1 };
                }
                _ if is_word_byte(c) || c == b'\\' => i = self.word(i)?,
                _ => {
                    self.token(Token::Punct(c));
                    i += 1;
                }
            }
        }
        while self.stack.len() > 1 {
            self.pop()?;
        }
        let cost = self.top().cost();
        if cost > LIMIT { Err(too_deep(cost)) } else { Ok(cost) }
    }

    /// A word at `at`: a name, number or keyword, `\` escapes (PN_LOCAL_ESC) included. Returns
    /// where scanning continues.
    fn word(&mut self, at: usize) -> Result<usize> {
        let b = self.b;
        let mut i = at;
        let mut dashes = 0;
        while i < b.len() {
            match b[i] {
                b'\\' => i += 2,
                b'-' => {
                    dashes += 1;
                    i += 1;
                }
                c if is_word_byte(c) => i += 1,
                _ => break,
            }
        }
        // `-` inside a prefixed name is no operator, but in an expression it may be (`1-2`).
        for _ in 0..dashes {
            self.operator(b'-')?;
        }
        let kw = keyword(&b[at..i.min(b.len())]);
        match kw {
            Keyword::Clause => self.top().clause = true,
            Keyword::Where => self.top().clause = false,
            _ => {}
        }
        self.token(Token::Word(kw));
        Ok(i)
    }

    /// Counts an operator byte by the frame's mode.
    fn operator(&mut self, c: u8) -> Result<()> {
        match (self.top().mode, c) {
            (Mode::Expr, b'|' | b'&' | b'+' | b'-' | b'*' | b'/' | b'!') => self.add(W_OP),
            (Mode::Pattern, b'/' | b'|' | b'^' | b'*' | b'+' | b'!' | b'?') => {
                // Also inside a path group or a blank-node list: the path triple is one more
                // element of the enclosing group's join chain.
                let group = self.stack.iter().rposition(|f| f.close == b'}' || f.close == 0).unwrap_or(0);
                self.add_at(group, W_PATH)
            }
            _ => Ok(()),
        }
    }

    fn open_paren(&mut self) -> Result<()> {
        let top = self.top();
        let (mode, root, clause) = (top.mode, top.root, top.clause);
        let child = match mode {
            Mode::Data => Mode::Data,
            Mode::Expr => Mode::Expr,
            Mode::Pattern if root || clause || self.constraint => Mode::Expr,
            Mode::Pattern => Mode::Pattern,
        };
        if mode == Mode::Pattern && child == Mode::Expr {
            // FILTER / BIND / projection element: one more filter / extend in the chain.
            self.add(W_CHILD)?;
        }
        self.push(child, b')', W_PAREN)?;
        self.token(Token::Punct(b'('));
        Ok(())
    }

    fn open_brace(&mut self) -> Result<()> {
        let top = self.top();
        let (mode, root) = (top.mode, top.root);
        top.clause = false;
        // Quad data and templates only follow their keyword at the top level.
        let child = if mode == Mode::Data || (root && self.prev == Token::Word(Keyword::Template)) {
            Mode::Data
        } else {
            Mode::Pattern
        };
        if mode == Mode::Pattern && child == Mode::Pattern && !root {
            // A child group, OPTIONAL, MINUS, UNION branch, GRAPH, VALUES, EXISTS, …: one more
            // join / left join / union in the chain. (Update operations are a flat list.)
            self.add(W_CHILD)?;
        }
        self.push(child, b'}', W_BRACE)?;
        self.token(Token::Punct(b'{'));
        Ok(())
    }

    /// `<` at `at`: an IRI, the comparison operator, or both (see the module docs). Returns
    /// where scanning continues.
    fn angle(&mut self, at: usize) -> Result<usize> {
        let Some(end) = iri_end(self.b, at) else {
            // Not an IRI: the operator (or a parse error).
            self.token(Token::Punct(b'<'));
            return Ok(at + 1);
        };
        if self.top().mode == Mode::Expr && self.prev.may_end_operand() {
            // Both readings: as an expression the span must tokenize the same way (no
            // literal, comment or escape may start in it, its brackets close in it), and its
            // cost counts.
            let span = &self.b[at + 1..end];
            if span.iter().any(|c| matches!(c, b'\'' | b'#' | b'\\')) || !balanced(span) {
                return Err(Error::Unsupported(format!(
                    "ambiguous '<' at byte {at}: it reads both as a comparison and as the start of an IRI; \
                     write a space after a '<' comparison"
                )));
            }
            let cost = Scanner::new(span, Mode::Expr, false).run()?;
            self.child_cost(cost)?;
        }
        self.token(Token::Iri);
        Ok(end + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cost(text: &str) -> u32 {
        Scanner::new(text.as_bytes(), Mode::Pattern, true).run().unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    fn refused(text: &str) -> String {
        match check(text) {
            Ok(()) => panic!("accepted: {text}"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn ordinary_requests_are_cheap() {
        for q in [
            "SELECT * WHERE { ?s ?p ?o }",
            "PREFIX ex: <http://ex/> SELECT ?n (COUNT(*) AS ?c) WHERE { ?p ex:name ?n OPTIONAL { ?p ex:age ?a } \
             FILTER(?a > 20 && REGEX(?n, \"^(A|B)[a-z]*\", \"i\")) } GROUP BY ?n HAVING (COUNT(*) > 1) ORDER BY DESC(?c)",
            "SELECT * WHERE { ?s <http://ex/p>/<http://ex/q>* ?o . ?o ^<http://ex/r>|!(<http://ex/a>|^<http://ex/b>) ?x }",
            "ASK { ?s ?p \"\"\"a long (( [[ {{ literal\"\"\" FILTER NOT EXISTS { ?s ?q ?r } }",
            "CONSTRUCT { ?s ?p [ <http://ex/q> (1 2 3) ] } WHERE { ?s ?p ?o } # comment ((((((",
            "INSERT DATA { GRAPH <http://ex/g> { <http://ex/a> <http://ex/b> \"x\"^^<http://www.w3.org/2001/XMLSchema#string> } }",
        ] {
            let c = cost(q);
            assert!(c < 100, "{q}: {c}");
        }
    }

    #[test]
    fn brackets_in_literals_iris_and_comments_cost_nothing() {
        let base = cost("SELECT * WHERE { ?s ?p ?o }");
        let deep = "(".repeat(5000);
        for q in [
            format!("SELECT * WHERE {{ ?s ?p \"{deep}\" }}"),
            format!("SELECT * WHERE {{ ?s ?p '{deep}' }}"),
            format!("SELECT * WHERE {{ ?s ?p '''{deep}\n''' }}"),
            format!("SELECT * WHERE {{ ?s ?p \"\"\"{deep}\"\"\" }}"),
            format!("SELECT * WHERE {{ ?s ?p ?o }} # {deep}"),
            format!("SELECT * WHERE {{ ?s ?p <http://ex/{deep}> }}"),
            format!("SELECT * WHERE {{ ?s ?p \"a\\\"{deep}\" }}"),
        ] {
            assert_eq!(cost(&q), base, "{q:.60}");
        }
    }

    #[test]
    fn deep_nesting_and_long_chains_are_refused() {
        let n = 3000;
        for q in [
            format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}?o{}) }}", "(".repeat(n), ")".repeat(n)),
            format!("SELECT * WHERE {}?s ?p ?o{}", "{ ".repeat(n), " }".repeat(n)),
            format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o{} > 0) }}", " + 1".repeat(n)),
            format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o{} > 0) }}", "+1".repeat(n)),
            format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o = 0{}) }}", " || ?o = 1".repeat(n)),
            format!("SELECT * WHERE {{ {{}}{} }}", " UNION {}".repeat(n)),
            format!("SELECT * WHERE {{ ?s ?p ?o{} }}", " OPTIONAL { ?s ?p ?o }".repeat(n)),
            format!("SELECT * WHERE {{ ?s ?p ?o{} }}", " FILTER(?o)".repeat(n)),
            format!("SELECT * WHERE {{ {} }}", "?s <http://ex/p>? ?o . ".repeat(n)),
            format!("SELECT * WHERE {{ ?s <http://ex/p>{} ?o }}", "/<http://ex/p>".repeat(n)),
            format!("INSERT DATA {{ <http://ex/a> <http://ex/p> {}1{} }}", "( ".repeat(n), " )".repeat(n)),
            format!("INSERT DATA {{ <http://ex/a> <http://ex/p> {}1{} }}", "[ <http://ex/q> ".repeat(n), " ]".repeat(n)),
            format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}1{}) }}", "!(".repeat(n), ")".repeat(n)),
            // Unclosed brackets cost as much as closed ones.
            format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}", "(".repeat(n)),
        ] {
            let e = refused(&q);
            assert!(e.contains("nests too deeply"), "{q:.60}: {e}");
        }
    }

    #[test]
    fn flat_data_and_many_update_operations_are_accepted() {
        let triples: String = (0..20_000).map(|i| format!("<http://ex/s{i}> <http://ex/p> [ <http://ex/q> (1 2) ] .\n")).collect();
        check(&format!("INSERT DATA {{ {triples} }}")).unwrap();
        check(&format!("CONSTRUCT {{ {triples} }} WHERE {{ ?s ?p ?o }}")).unwrap();
        let graphs: String = (0..5000).map(|i| format!("GRAPH <http://ex/g{i}> {{ <http://ex/a> <http://ex/b> {i} }}\n")).collect();
        check(&format!("INSERT DATA {{ {graphs} }}")).unwrap();
        let ops: String = (0..5000).map(|i| format!("DELETE WHERE {{ <http://ex/s{i}> ?p ?o }} ;\n")).collect();
        check(&ops).unwrap();
        let rows: String = (0..20_000).map(|i| format!("(<http://ex/a{i}#x> {i})")).collect();
        check(&format!("SELECT * WHERE {{ VALUES (?x ?n) {{ {rows} }} }}")).unwrap();
        let alternatives: String = (0..400).map(|i| format!(" || ?x = <http://ex/a{i}#x>")).collect();
        check(&format!("SELECT * WHERE {{ ?s ?p ?x FILTER(?x = 0{alternatives}) }}")).unwrap();
        let triples: String = (0..2000).map(|i| format!("?s <http://ex/p{i}> ?o{i} .\n")).collect();
        check(&format!("SELECT * WHERE {{ {triples} }}")).unwrap();
    }

    #[test]
    fn the_limit_sits_between_these_shapes() {
        let filters = |n: usize| format!("SELECT * WHERE {{ ?s ?p ?o{} }}", " FILTER(?o != 1)".repeat(n));
        assert!(check(&filters(200)).is_ok());
        assert!(check(&filters(300)).is_err());
        let parens = |n: usize| format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}?o{}) }}", "(".repeat(n), ")".repeat(n));
        assert!(check(&parens(150)).is_ok());
        assert!(check(&parens(250)).is_err());
    }

    #[test]
    fn an_unterminated_long_literal_falls_back_to_short_ones() {
        // spargebra reads `'''x` (no closing `'''`) as `''` then `'x…'`: what follows counts.
        let deep = format!("SELECT * WHERE {{ ?s ?p '''x' FILTER({}?o{}) }}", "(".repeat(400), ")".repeat(400));
        assert!(refused(&deep).contains("nests too deeply"));
        assert_eq!(string_end(b"'''a'''", 0), Some(7));
        assert_eq!(string_end(b"'''a''''", 0), Some(7), "the first ''' closes");
        assert_eq!(string_end(b"'''a''b'''", 0), Some(10));
        assert_eq!(string_end(b"'''a", 0), Some(2), "'' then a new literal");
        assert_eq!(string_end(b"'''a\\q'''", 0), Some(2), "an invalid escape fails the long literal");
        assert_eq!(string_end(b"'a\\'b'", 0), Some(6));
        assert_eq!(string_end(b"'a\\u00e9'", 0), Some(9));
        assert_eq!(string_end(b"'a\nb'", 0), None);
        assert_eq!(string_end(b"\"a'b\"", 0), Some(5));
    }

    #[test]
    fn a_comparison_without_spaces_is_read_both_ways() {
        // `<?b&&?c>` is also a plausible IRI: counted as an expression, accepted.
        assert!(cost("SELECT * WHERE { ?a ?p ?b FILTER(?a<?b&&?c>?d) }") < 30);
        // Nesting hidden in an IRI-looking span after an operand still counts.
        let hidden = format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o <{}1{}> 0) }}", "(".repeat(400), ")".repeat(400));
        assert!(refused(&hidden).contains("nests too deeply"));
        // A span that could open a literal or a comment as an expression is refused.
        for q in [
            "SELECT * WHERE { ?s ?p ?o FILTER(?o <a'> ' ) }",
            "SELECT * WHERE { ?s ?p ?o FILTER(?o <a#> ) }",
            "SELECT * WHERE { ?s ?p ?o FILTER(?o <f(> 2)) }",
        ] {
            assert!(refused(q).contains("ambiguous '<'"), "{q}");
        }
        // Not after an operand, or outside expressions, `<…>` is just an IRI.
        for q in [
            "SELECT * WHERE { ?s ?p ?o FILTER(?o = <http://ex/a#b'c>) }",
            "SELECT * WHERE { ?s ?p ?o FILTER(?o IN (<http://ex/a#1>, <http://ex/a#2>)) }",
            "SELECT * WHERE { ?s ?p ?o FILTER(?o < <http://ex/a#b>) }",
            "SELECT * WHERE { ?s ?p (<http://ex/a#1> <http://ex/a#2>) FILTER(<http://ex/f#g>(?o)) }",
            "SELECT * WHERE { ?s ?p ?o } ORDER BY ?o <http://ex/f#g>(?s)",
        ] {
            check(q).unwrap_or_else(|e| panic!("{q}: {e}"));
        }
    }

    #[test]
    fn frames_are_classified_by_keywords() {
        // A FILTER function call and a projection are expressions; a collection is not.
        // `-` is an operator in expressions only (a negative number in a collection).
        let ops = " - 1".repeat(60);
        assert!(cost(&format!("SELECT * WHERE {{ ?s ?p ?o FILTER <http://ex/f>(?o{ops}) }}")) > 60);
        assert!(cost(&format!("SELECT * WHERE {{ ?s ?p ?o FILTER ex:f(?o{ops}) }}")) > 60);
        assert!(cost(&format!("SELECT ?a (?b{ops} AS ?c) WHERE {{ ?s ?p ?o }}")) > 60);
        assert!(cost(&format!("SELECT * WHERE {{ {{ SELECT ?a (1 AS ?x) (?b{ops} AS ?c) {{}} }} }}")) > 60);
        assert!(cost(&format!("SELECT * WHERE {{ ?s ?p (?o{ops}) }}")) < 60, "a collection");
        // `INSERT DATA` and templates are flat.
        let groups = "{ <http://ex/a> <http://ex/b> 1 } ".repeat(200);
        assert!(cost(&format!("INSERT DATA {{ GRAPH <http://ex/g> {groups} }}")) < 100);
        assert!(cost(&format!("SELECT * WHERE {{ {groups} }}")) > 800);
    }

    /// Shapes where the parser reads an expression (or a chain) that a looser scanner would
    /// miss: each hides 400 levels of nesting behind `<…>` after an operand, or builds a long
    /// chain.
    #[test]
    fn expressions_and_chains_are_found_however_they_are_spelled() {
        let hidden = format!("?o <{}1{}> 0", "(".repeat(400), ")".repeat(400));
        for q in [
            // Function names with `-`, `.` and escapes; keywords glued to a number or a name.
            format!("SELECT * WHERE {{ ?s ?p ?o FILTER ex:my-func({hidden}) }}"),
            format!("SELECT * WHERE {{ ?s ?p ?o FILTER ex:my.func({hidden}) }}"),
            format!("SELECT * WHERE {{ ?s ?p ?o FILTER ex:f\\.g\\-h({hidden}) }}"),
            format!("SELECT * WHERE {{ ?s ?p 1FILTER({hidden}) }}"),
            format!("SELECT * WHERE {{ ?s ?p ex:a.b.FILTER({hidden}) }}"),
            format!("SELECT * WHERE {{ ?s ?p ?o filter # comment\n <http://ex/f> ({hidden}) }}"),
            format!("SELECT * WHERE {{ ?s ?p ?o BIND(({hidden}) AS ?x) }}"),
            format!("SELECT * WHERE {{ {{ SELECT ?a (1 AS ?b) ({hidden} AS ?c) {{}} }} }}"),
            format!("SELECT * WHERE {{ ?s ?p ?o }} GROUP BY ?s ({hidden})"),
        ] {
            assert!(refused(&q).contains("nests too deeply"), "{q:.70}");
        }
        for q in [
            // Parenthesized paths and paths in blank-node lists join the group's chain.
            format!("SELECT * WHERE {{ {} }}", "?s (<http://ex/p>/<http://ex/q>) ?o . ".repeat(300)),
            format!("SELECT * WHERE {{ {} }}", "[ <http://ex/p>* ?o ] . ".repeat(300)),
            // `DATA` only makes a top-level `{` flat.
            format!("SELECT * WHERE {{ ?s ?p ex:o.DATA {{ {} }} }}", "{} ".repeat(300)),
            format!("SELECT * WHERE {{ ?s ?p ?o DATA {{ {} }} }}", "{} ".repeat(300)),
        ] {
            assert!(refused(&q).contains("nests too deeply"), "{q:.70}");
        }
        // A FILTER's pending `(` ends with its first other token.
        assert!(cost("SELECT * WHERE { FILTER(?x) ?s ?p (<http://ex/a#1> <http://ex/a#2>) }") < 40);
    }

    /// [`iri_end`] relies on spargebra (through oxiri) refusing these bytes inside `<…>`:
    /// otherwise skipping a span the scanner rejects could hide structure.
    #[test]
    fn spargebra_rejects_the_bytes_that_end_an_iri_span() {
        for c in [" ", "\t", "\u{1}", "\"", "{", "}", "|", "^", "`", "<"] {
            let q = format!("SELECT * WHERE {{ ?s ?p <http://ex/a{c}b> }}");
            assert!(sq_plan::parse_query(&q, None).is_err(), "{c:?}");
            assert!(sq_plan::parse_query(&q, Some("http://base/")).is_err(), "{c:?} (relative)");
        }
        assert!(sq_plan::parse_query("SELECT * WHERE { ?s ?p <http://ex/a\\u0041b> }", None).is_ok());
    }
}
