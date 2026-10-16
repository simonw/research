//! SQLite's compile-time limits, checked on the finished statement so that every query the
//! translator accepts also prepares (anything over a limit is `Error::Unsupported`).
//!
//! The host links the system libsqlite3 3.45.1 with its default limits:
//! * the LALR parser has a fixed 100-entry stack (3.46 made it grow): every open parenthesis,
//!   subquery, CASE and pending clause holds entries on it;
//! * expression trees are at most 1000 deep (`SQLITE_MAX_EXPR_DEPTH`), and a subquery adds the
//!   depth of its own expressions to the expression that contains it;
//! * a compound SELECT has at most 500 terms, a function call at most 127 arguments and a
//!   result at most 2000 columns;
//! * a statement binds at most 250000 parameters (`SQLITE_MAX_VARIABLE_NUMBER`), and every
//!   distinct constant of the query is one.
//!
//! Besides, statements are kept to [`MAX_SQL_BYTES`] (SQLite accepts up to 1 GB, but nothing
//! that large is worth preparing): the translator checks the size of every pattern as it goes
//! ([`check_size`]), so a query that would need a huge statement fails before building it.
//!
//! The 64-tables-per-join limit depends on SQLite's subquery flattening, which the text does
//! not show; the translator guarantees it by construction (`Translator::cap_tables`).
//!
//! # Parser-stack model
//!
//! The scan is a conservative model, not a parser: the estimated stack at any point is
//! [`ROOT_STACK`] plus, for every enclosing nesting level, the cost of opening it
//! ([`Kind::cost`]) and, for every enclosing statement or subquery, the extra entries its
//! current clause holds ([`Clause::extra`]). The numbers were calibrated on SQLite 3.45.1 by
//! measuring how many `(((…)))` still fit at each position the translator emits a nested
//! construct, and at each depth of every construct nested in itself. Measured (model): a
//! SELECT list 7 (8); a WHERE with pending `… AND x =` 12 (13); an ON clause after a derived
//! table with pending `… AND x =` 17 (18); a window `OVER (ORDER BY f(…), ` 18 (26); each
//! further level of derived table 6 (7), of EXISTS in a WHERE 9 (12), of `(NOT EXISTS (…))`
//! 11 (19), of function call 7 (7), of CASE 6 (10). The unit tests replay every measurement
//! (`tests::limits`).
use sq_core::Result;

use crate::translate::unsupported;

/// Stack entries of a statement with no nesting (SQLite's own bottom entries and a SELECT list
/// with a pending operator).
const ROOT_STACK: u32 = 8;
/// SQLite 3.45's `YYSTACKDEPTH`.
const PARSER_STACK: u32 = 100;

/// Expression-depth units charged per nesting level besides its AND / OR operators (every
/// other operator the translator emits sits in a short, fixed-shape subtree of its level).
const LEVEL_DEPTH: u32 = 8;
/// `SQLITE_MAX_EXPR_DEPTH`.
const MAX_EXPR_DEPTH: u32 = 1000;
/// `SQLITE_MAX_COMPOUND_SELECT`.
const MAX_COMPOUND_TERMS: u32 = 500;
/// `SQLITE_MAX_FUNCTION_ARG` (3.45).
const MAX_FUNCTION_ARGS: u32 = 127;
/// `SQLITE_MAX_COLUMN`.
const MAX_COLUMNS: u32 = 2000;
/// `SQLITE_MAX_VARIABLE_NUMBER` (3.32 and later).
const MAX_PARAMS: usize = 250_000;
/// The largest statement the translator produces.
pub(crate) const MAX_SQL_BYTES: usize = 8 << 20;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Kind {
    /// The statement or a parenthesized subquery (`(SELECT`, `(WITH`, `(VALUES`).
    Query,
    /// A function call's argument list (`name(`).
    Call,
    /// A window definition (`OVER (`).
    Window,
    /// Any other parenthesis.
    Paren,
    /// `CASE … END`.
    Case,
}

impl Kind {
    /// Stack entries charged for opening the level (with the operators pending inside it,
    /// except for subqueries, whose clauses are charged separately).
    fn cost(self) -> u32 {
        match self {
            Kind::Query | Kind::Call | Kind::Paren => 7,
            Kind::Case => 10,
            Kind::Window => 11,
        }
    }
}

/// The clause a statement or subquery is in.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Clause {
    /// SELECT list, FROM, VALUES, CTE list.
    Head,
    Where,
    On,
    GroupBy,
    /// ORDER BY, LIMIT, OFFSET.
    Tail,
}

impl Clause {
    /// Extra entries the clause holds (with its pending operators).
    fn extra(self) -> u32 {
        match self {
            Clause::Head => 0,
            Clause::Where => 5,
            Clause::GroupBy => 6,
            Clause::Tail => 8,
            Clause::On => 10,
        }
    }
}

/// Extra entries once a statement is a compound (the previous terms stay on the stack as one
/// entry plus the operator) and once it has a WITH clause.
const COMPOUND_EXTRA: u32 = 3;
const WITH_EXTRA: u32 = 3;

/// One open nesting level.
#[derive(Debug)]
struct Level {
    kind: Kind,
    /// Estimated stack when the level opened.
    base: u32,
    clause: Clause,
    compound: bool,
    with: bool,
    /// AND / OR operators directly at this level, in the current clause's expression.
    ops: u32,
    /// Most AND / OR operators of an earlier clause's expression at this level (every clause
    /// and every term of a compound has its own expression tree).
    max_ops: u32,
    /// Deepest expression depth of a closed inner level.
    inner_depth: u32,
    /// Commas directly at this level (arguments, or columns of the current SELECT list).
    commas: u32,
    /// UNION / EXCEPT / INTERSECT directly at this level.
    compounds: u32,
    /// The commas so far separate result columns (a SELECT list is being read).
    in_select_list: bool,
}

impl Level {
    fn new(kind: Kind, base: u32) -> Level {
        Level {
            kind,
            base,
            clause: Clause::Head,
            compound: false,
            with: false,
            ops: 0,
            max_ops: 0,
            inner_depth: 0,
            commas: 0,
            compounds: 0,
            in_select_list: false,
        }
    }

    /// Estimated stack inside this level at its current position.
    fn stack(&self) -> u32 {
        let mut s = self.base;
        if self.kind == Kind::Query {
            s += self.clause.extra();
            s += if self.compound { COMPOUND_EXTRA } else { 0 };
            s += if self.with { WITH_EXTRA } else { 0 };
        }
        s
    }

    fn depth(&self) -> u32 {
        self.ops.max(self.max_ops) + LEVEL_DEPTH + self.inner_depth
    }
}

/// Scan state.
struct Scan {
    levels: Vec<Level>,
    max_stack: u32,
}

impl Scan {
    fn top(&mut self) -> &mut Level {
        self.levels.last_mut().expect("the statement level")
    }

    fn open(&mut self, kind: Kind) {
        let base = self.top().stack() + kind.cost();
        self.max_stack = self.max_stack.max(base);
        self.levels.push(Level::new(kind, base));
    }

    /// Closes the innermost level, folding its expression depth into its parent.
    fn close(&mut self) -> Result<()> {
        let level = self.levels.pop().expect("an open level");
        check_level(&level)?;
        let depth = level.depth();
        if depth > MAX_EXPR_DEPTH {
            return Err(unsupported("expressions too deep for SQLite"));
        }
        let parent = self.top();
        parent.inner_depth = parent.inner_depth.max(depth);
        Ok(())
    }

    fn keyword(&mut self, word: &str) -> Result<()> {
        let top = self.top();
        let query = top.kind == Kind::Query;
        let clause = match word {
            "AND" | "OR" => {
                top.ops += 1;
                return Ok(());
            }
            "CASE" => {
                self.open(Kind::Case);
                return Ok(());
            }
            "END" if top.kind == Kind::Case => return self.close(),
            "UNION" | "EXCEPT" | "INTERSECT" if query => {
                top.compounds += 1;
                top.compound = true;
                top.max_ops = top.max_ops.max(top.ops);
                top.ops = 0;
                return Ok(());
            }
            "WITH" if query => {
                top.with = true;
                return Ok(());
            }
            "SELECT" if query => {
                top.in_select_list = true;
                top.commas = 0;
                Clause::Head
            }
            "FROM" | "JOIN" | "VALUES" if query => Clause::Head,
            "WHERE" if query => Clause::Where,
            "ON" if query => Clause::On,
            "GROUP" if query => Clause::GroupBy,
            "ORDER" | "LIMIT" | "OFFSET" if query => Clause::Tail,
            _ => return Ok(()),
        };
        if word != "SELECT" {
            top.in_select_list = false;
        }
        top.clause = clause;
        top.max_ops = top.max_ops.max(top.ops);
        top.ops = 0;
        let stack = top.stack();
        self.max_stack = self.max_stack.max(stack);
        Ok(())
    }
}

/// Checks a statement under construction (or its estimated size) against [`MAX_SQL_BYTES`].
pub(crate) fn check_size(bytes: usize) -> Result<()> {
    if bytes > MAX_SQL_BYTES {
        return Err(unsupported(format!("queries whose SQL exceeds {} MiB", MAX_SQL_BYTES >> 20)));
    }
    Ok(())
}

/// Checks the number of parameters a statement binds.
pub(crate) fn check_params(n: usize) -> Result<()> {
    if n > MAX_PARAMS {
        return Err(unsupported(format!("more than {MAX_PARAMS} distinct constants (SQL parameters)")));
    }
    Ok(())
}

/// Checks the statement against the limits above.
pub(crate) fn check(sql: &str) -> Result<()> {
    check_size(sql.len())?;
    let scan = scan(sql)?;
    let root = &scan.levels[0];
    check_level(root)?;
    if root.depth() > MAX_EXPR_DEPTH {
        return Err(unsupported("expressions too deep for SQLite"));
    }
    if scan.max_stack > PARSER_STACK {
        return Err(unsupported("query nesting too deep for SQLite's parser"));
    }
    Ok(())
}

/// Scans the whole statement; on return only the statement level is open.
fn scan(sql: &str) -> Result<Scan> {
    let bytes = sql.as_bytes();
    let mut scan = Scan { levels: vec![Level::new(Kind::Query, ROOT_STACK)], max_stack: ROOT_STACK };
    let mut last_word = "";
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\'' {
            // A string literal ('' escapes a quote).
            i += 1;
            while i < bytes.len() && !(bytes[i] == b'\'' && bytes.get(i + 1) != Some(&b'\'')) {
                i += if bytes[i] == b'\'' { 2 } else { 1 };
            }
            i += 1;
            last_word = "";
            continue;
        }
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            last_word = &sql[start..i];
            scan.keyword(last_word)?;
            continue;
        }
        match b {
            b'(' => {
                let rest = sql[i + 1..].trim_start();
                // The translator writes `name(` for calls and `KEYWORD (` otherwise.
                let after_name = i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                let kind = if ["SELECT", "WITH", "VALUES"].iter().any(|k| rest.starts_with(k)) {
                    Kind::Query
                } else if after_name {
                    Kind::Call
                } else if last_word == "OVER" {
                    Kind::Window
                } else {
                    Kind::Paren
                };
                scan.open(kind);
            }
            b')' => {
                // Close a CASE left open by malformed text, then the parenthesis itself.
                while scan.levels.len() > 1 && scan.top().kind == Kind::Case {
                    scan.close()?;
                }
                if scan.levels.len() > 1 {
                    scan.close()?;
                }
            }
            b',' => {
                let top = scan.top();
                top.commas += 1;
                if top.in_select_list && top.commas + 1 > MAX_COLUMNS {
                    return Err(unsupported(format!("more than {MAX_COLUMNS} result columns")));
                }
            }
            _ => {}
        }
        if !b.is_ascii_whitespace() {
            last_word = "";
        }
        i += 1;
    }
    while scan.levels.len() > 1 {
        scan.close()?;
    }
    Ok(scan)
}

/// Per-level limits.
fn check_level(level: &Level) -> Result<()> {
    if level.compounds + 1 > MAX_COMPOUND_TERMS {
        return Err(unsupported(format!("more than {MAX_COMPOUND_TERMS} terms in a compound SELECT")));
    }
    if level.kind == Kind::Call && level.commas + 1 > MAX_FUNCTION_ARGS {
        return Err(unsupported(format!("more than {MAX_FUNCTION_ARGS} arguments in a function call")));
    }
    Ok(())
}

/// The estimated parser stack of a statement.
#[cfg(test)]
pub(crate) fn estimated_stack(sql: &str) -> u32 {
    scan(sql).expect("within the per-level limits").max_stack
}
