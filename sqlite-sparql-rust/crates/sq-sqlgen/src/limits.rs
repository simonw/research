//! SQLite's compile-time limits, checked on the finished statement so that every query the
//! translator accepts also prepares (anything over a limit is `Error::Unsupported`).
//!
//! The host links the system libsqlite3 3.45.1 with its default limits:
//! * the LALR parser has a fixed 100-entry stack (3.46 made it grow): every open parenthesis,
//!   subquery and CASE holds entries on it until it closes;
//! * expression trees are at most 1000 deep (`SQLITE_MAX_EXPR_DEPTH`), and a subquery adds the
//!   depth of its own expressions to the expression that contains it;
//! * a compound SELECT has at most 500 terms, a function call at most 127 arguments and a
//!   result at most 2000 columns.
//!
//! The 64-tables-per-join limit depends on SQLite's subquery flattening, which the text does
//! not show; the translator guarantees it by construction (`Translator::cap_tables`).
//!
//! The scan is a conservative model, not a parser. Each nesting level is charged at least the
//! parser-stack entries the most demanding context of its kind holds, as measured on 3.45.1
//! by nesting `(((…)))` at every position the translator emits (statement prefix and clause,
//! pending operators, the level's own parenthesis): a statement with a SELECT list at most 7
//! entries, a WHERE with pending `AND … =` 12, an ON clause after a derived table 15, a window
//! `ORDER BY` 16; a parenthesis (function arguments, `IS NULL OR … =`, IN lists) at most 6
//! more; a CASE up to `WHEN … AND … BETWEEN … AND` 9 more; a subquery opened from a WHERE,
//! FROM or ON clause at most 13 more.
use sq_core::Result;

use crate::translate::unsupported;

/// Parser-stack entries charged to the statement itself (its clauses and pending operators).
const ROOT_COST: u32 = 18;
/// … to a parenthesized subquery (`(SELECT`, `(WITH`, `(VALUES`).
const QUERY_COST: u32 = 14;
/// … to any other parenthesis (grouping, function call, IN list).
const PAREN_COST: u32 = 7;
/// … to a `CASE … END`.
const CASE_COST: u32 = 10;
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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Kind {
    /// The statement or a parenthesized subquery.
    Query,
    /// A function call's argument list.
    Call,
    /// Any other parenthesis.
    Paren,
    /// `CASE … END`.
    Case,
}

/// One open nesting level.
#[derive(Debug)]
struct Level {
    kind: Kind,
    /// Parser-stack estimate up to and including this level.
    stack: u32,
    /// AND / OR operators directly at this level.
    ops: u32,
    /// Deepest expression depth of a closed inner level.
    inner_depth: u32,
    /// Commas directly at this level (arguments, select-list columns, VALUES cells).
    commas: u32,
    /// UNION / EXCEPT / INTERSECT directly at this level.
    compounds: u32,
    /// The commas directly at this level so far separate result columns (a SELECT list).
    in_select_list: bool,
}

impl Level {
    fn new(kind: Kind, stack: u32) -> Level {
        Level { kind, stack, ops: 0, inner_depth: 0, commas: 0, compounds: 0, in_select_list: false }
    }

    fn depth(&self) -> u32 {
        self.ops + LEVEL_DEPTH + self.inner_depth
    }
}

/// Checks the statement against the limits above.
pub(crate) fn check(sql: &str) -> Result<()> {
    let bytes = sql.as_bytes();
    let mut levels = vec![Level::new(Kind::Query, ROOT_COST)];
    let mut max_stack = ROOT_COST;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\'' {
            // A string literal ('' escapes a quote).
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                        continue;
                    }
                    break;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &sql[start..i];
            let top = levels.last_mut().expect("the statement level");
            match word {
                "AND" | "OR" => top.ops += 1,
                "UNION" | "EXCEPT" | "INTERSECT" => top.compounds += 1,
                "SELECT" => {
                    top.in_select_list = true;
                    top.commas = 0;
                }
                "FROM" | "WHERE" | "GROUP" | "ORDER" | "LIMIT" => top.in_select_list = false,
                "CASE" => {
                    let stack = top.stack + CASE_COST;
                    levels.push(Level::new(Kind::Case, stack));
                    max_stack = max_stack.max(stack);
                }
                "END" if top.kind == Kind::Case => close(&mut levels)?,
                _ => {}
            }
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
                } else {
                    Kind::Paren
                };
                let top = levels.last().expect("the statement level");
                let stack = top.stack + if kind == Kind::Query { QUERY_COST } else { PAREN_COST };
                levels.push(Level::new(kind, stack));
                max_stack = max_stack.max(stack);
            }
            b')' => {
                // Close a CASE left open by malformed text, then the parenthesis itself.
                while levels.len() > 1 && levels.last().is_some_and(|l| l.kind == Kind::Case) {
                    close(&mut levels)?;
                }
                if levels.len() > 1 {
                    close(&mut levels)?;
                }
            }
            b',' => {
                let top = levels.last_mut().expect("the statement level");
                top.commas += 1;
                if top.in_select_list && top.commas + 1 > MAX_COLUMNS {
                    return Err(unsupported(format!("more than {MAX_COLUMNS} result columns")));
                }
            }
            _ => {}
        }
        i += 1;
    }
    while levels.len() > 1 {
        close(&mut levels)?;
    }
    let root = &levels[0];
    check_level(root)?;
    if max_stack > PARSER_STACK {
        return Err(unsupported("query nesting too deep for SQLite's parser"));
    }
    if root.depth() > MAX_EXPR_DEPTH {
        return Err(unsupported("expressions too deep for SQLite"));
    }
    Ok(())
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

/// Closes the innermost level, folding its expression depth into its parent.
fn close(levels: &mut Vec<Level>) -> Result<()> {
    let level = levels.pop().expect("an open level");
    check_level(&level)?;
    let depth = level.depth();
    if depth > MAX_EXPR_DEPTH {
        return Err(unsupported("expressions too deep for SQLite"));
    }
    let parent = levels.last_mut().expect("the statement level");
    parent.inner_depth = parent.inner_depth.max(depth);
    Ok(())
}
