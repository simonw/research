//! SQL text building: parameters, column bindings and flat SELECT blocks.
//!
//! A [`Block`] is one `SELECT … FROM … WHERE …` under construction: FROM items (quad table
//! aliases, derived tables and LEFT JOINs), WHERE conjuncts and, per slot, the SQL expression
//! holding its `TermId` ([`Col`]). Basic graph patterns, joins, filters, OPTIONAL, MINUS and
//! BIND extend a block in place, so SQLite plans a whole group as one join; everything else
//! (UNION, VALUES, sub-selects, GROUP BY, paths) becomes a derived table inside it.
use std::collections::BTreeMap;
use std::fmt::Write as _;

use sq_core::{FxHashMap, Slot};

/// Positional parameters: one `?N` per distinct value, `values[N - 1]` binds it.
#[derive(Debug, Default)]
pub(crate) struct Params {
    values: Vec<i64>,
    index: FxHashMap<i64, usize>,
}

impl Params {
    /// The placeholder binding `v` (values are deduplicated, so a constant used twice is one
    /// parameter).
    pub fn get(&mut self, v: i64) -> String {
        let n = *self.index.entry(v).or_insert_with(|| {
            self.values.push(v);
            self.values.len()
        });
        format!("?{n}")
    }

    /// The final statement and its parameter values: placeholders are renumbered by first use,
    /// dropping values allocated for fragments the translation discarded (an empty UNION
    /// branch, a pattern that cannot match). Generated SQL contains `?` only in placeholders.
    pub fn finish(self, sql: &str) -> (String, Vec<i64>) {
        let mut out = String::with_capacity(sql.len());
        let mut values = Vec::new();
        let mut renumbered: FxHashMap<usize, usize> = FxHashMap::default();
        let mut rest = sql;
        while let Some(at) = rest.find('?') {
            out.push_str(&rest[..at]);
            let digits = rest[at + 1..].bytes().take_while(u8::is_ascii_digit).count();
            let old: usize = rest[at + 1..at + 1 + digits].parse().expect("placeholders are ?N");
            let new = *renumbered.entry(old).or_insert_with(|| {
                values.push(self.values[old - 1]);
                values.len()
            });
            let _ = write!(out, "?{new}");
            rest = &rest[at + 1 + digits..];
        }
        out.push_str(rest);
        (out, values)
    }
}

/// Where a slot's value lives: an SQL expression of type INTEGER `TermId`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Col {
    pub sql: String,
    /// May be NULL (unbound) in some rows.
    pub nullable: bool,
}

impl Col {
    pub fn new(sql: impl Into<String>) -> Col {
        Col { sql: sql.into(), nullable: false }
    }

    pub fn nullable(sql: impl Into<String>) -> Col {
        Col { sql: sql.into(), nullable: true }
    }
}

/// Column name of a slot in derived tables and in the statement's result.
pub(crate) fn col_name(s: Slot) -> String {
    format!("v{s}")
}

/// Result column of a SELECT that binds no slot (SQL needs at least one column).
pub(crate) const PLACEHOLDER: &str = "NULL AS u";

/// One FROM-clause entry.
#[derive(Clone, Debug)]
pub(crate) enum Item {
    /// `main.rdf_quad AS alias`.
    Quad(String),
    /// `(sql) AS alias`.
    Derived { sql: String, alias: String },
    /// `LEFT JOIN <item> ON <on>` (never the first item).
    Left { item: Box<Item>, on: String },
}

impl Item {
    fn render(&self, out: &mut String, first: bool) {
        match self {
            Item::Quad(alias) => {
                out.push_str(if first { "" } else { ", " });
                let _ = write!(out, "main.rdf_quad AS {alias}");
            }
            Item::Derived { sql, alias } => {
                out.push_str(if first { "" } else { ", " });
                let _ = write!(out, "({sql}) AS {alias}");
            }
            Item::Left { item, on } => {
                out.push_str(" LEFT JOIN ");
                item.render(out, true);
                let _ = write!(out, " ON {on}");
            }
        }
    }
}

/// A flat SELECT under construction (see the module docs).
#[derive(Clone, Debug, Default)]
pub(crate) struct Block {
    pub items: Vec<Item>,
    pub conds: Vec<String>,
    /// Bound slots (the block's maybe-bound variables), ordered for deterministic SQL.
    pub cols: BTreeMap<Slot, Col>,
    /// Known to produce no rows (its only condition is `0`).
    pub empty: bool,
}

impl Block {
    /// One empty solution (`{}`).
    pub fn unit() -> Block {
        Block::default()
    }

    /// No solutions.
    pub fn empty() -> Block {
        Block { conds: vec!["0".into()], empty: true, ..Block::default() }
    }

    /// True if the block is a single plain FROM item (no LEFT JOIN) and every binding is a
    /// column of that item: it can then be the right operand of a LEFT JOIN as is (its
    /// bindings become NULL when nothing matches; a constant or computed binding would not).
    pub fn is_single_item(&self) -> bool {
        let alias = match self.items.as_slice() {
            [Item::Quad(alias) | Item::Derived { alias, .. }] => alias,
            _ => return false,
        };
        self.cols.values().all(|c| {
            c.sql.strip_prefix(alias.as_str()).and_then(|rest| rest.strip_prefix('.')).is_some_and(|col| {
                !col.is_empty() && col.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            })
        })
    }

    /// ` FROM …` (empty when there are no items).
    pub fn from_sql(&self) -> String {
        let mut out = String::new();
        if !self.items.is_empty() {
            out.push_str(" FROM ");
            for (i, item) in self.items.iter().enumerate() {
                item.render(&mut out, i == 0);
            }
        }
        out
    }

    /// ` WHERE …` (empty when there are no conditions).
    pub fn where_sql(&self) -> String {
        if self.conds.is_empty() { String::new() } else { format!(" WHERE {}", and_all(&self.conds)) }
    }

    /// `SELECT [DISTINCT] <out columns> FROM … WHERE …`; slots the block does not bind are
    /// NULL.
    pub fn render(&self, out: &[Slot], distinct: bool) -> String {
        let mut sql = String::from(if distinct { "SELECT DISTINCT " } else { "SELECT " });
        sql.push_str(&self.select_list(out));
        sql.push_str(&self.from_sql());
        sql.push_str(&self.where_sql());
        sql
    }

    /// `e1 AS v1, …` for `out`, or the placeholder column when `out` is empty.
    pub fn select_list(&self, out: &[Slot]) -> String {
        if out.is_empty() {
            return PLACEHOLDER.to_owned();
        }
        let mut list = String::new();
        for (i, s) in out.iter().enumerate() {
            if i > 0 {
                list.push_str(", ");
            }
            let expr = self.cols.get(s).map_or("NULL", |c| c.sql.as_str());
            let _ = write!(list, "{expr} AS {}", col_name(*s));
        }
        list
    }

    /// All bound slots, ascending.
    pub fn slots(&self) -> Vec<Slot> {
        self.cols.keys().copied().collect()
    }
}

/// Conjunction of SQL conditions (`1` when empty).
pub(crate) fn and_all(conds: &[String]) -> String {
    match conds.len() {
        0 => "1".to_owned(),
        1 => conds[0].clone(),
        _ => conds.join(" AND "),
    }
}

/// Disjunction of SQL conditions, parenthesized (`0` when empty).
pub(crate) fn or_all(conds: &[String]) -> String {
    match conds.len() {
        0 => "0".to_owned(),
        1 => conds[0].clone(),
        _ => format!("({})", conds.join(" OR ")),
    }
}
