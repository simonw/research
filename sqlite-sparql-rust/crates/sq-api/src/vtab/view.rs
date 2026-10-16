//! `sparql_view`: a SPARQL SELECT query as a virtual table with typed columns (§8.3b).
//!
//! ```sql
//! CREATE VIRTUAL TABLE temp.people USING sparql_view(
//!   'SELECT ?person ?name WHERE { ?person <http://ex/name> ?name }', engine=native, timeout_ms=2000);
//! -- declared: CREATE TABLE x("person","name","person__term" HIDDEN,"name__term" HIDDEN,
//! --                          "person__id" HIDDEN,"name__id" HIDDEN,"opts" HIDDEN)
//! SELECT name FROM people WHERE person__term = '<http://ex/alice>';
//! ```
//! Value columns hold the native mapping (§8.4), `__term` columns the N-Triples text and
//! `__id` columns the term's id in the store (NULL when unbound or not stored; a computed
//! term that is stored shows its stored id, see `Stream::public_id`). Equality constraints on
//! `__term` / `__id` are pushed down as top-level `FILTER(sameTerm(?v, c))` around the whole
//! query, so the result is exactly the SQL filter applied to the view's rows (the planner
//! turns a filter into a seed only where that is equivalent; an unbound value matches
//! nothing). Values that can never equal the column (wrong type, a non-canonical spelling, an
//! id that names no term) give an empty scan, as does a non-TEXT `opts` constraint.
//! Native-value columns are never pushed down: TEXT `'x'` is ambiguous between `<x>`, `"x"`
//! and `"x"@en`; nor are `__term` constraints under a collation other than BINARY
//! (`COLLATE NOCASE`, …), which SQLite then filters itself.
use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_int};
use std::sync::Arc;

use rusqlite::types::{Null, ValueRef};
use rusqlite::vtab::{
    Context, CreateVTab, Filters, IndexConstraintOp, IndexInfo, VTab, VTabConnection, VTabCursor, VTabKind,
    escape_double_quote, parameter, sqlite3_vtab, sqlite3_vtab_cursor,
};
use sq_core::codec;
use sq_core::ir::{Expr, Logical, QueryForm};
use sq_core::store::Dictionary;
use sq_core::{Error, Result, Tag, Term, TermId};

use super::{Source, Stream, col_used, column_error};
use crate::config::{Key, PanicSite, QuerySettings};
use crate::error::{IntoSql, sql_error};
use crate::guard::{guard, guard_drop, guard_eof};
use crate::query;
use crate::state::{ConnState, CursorToken};
use crate::value::{native, parse_term, write_ntriples};

#[repr(C)]
pub(crate) struct ViewTab {
    /// Base class; must be first.
    base: sqlite3_vtab,
    st: Arc<ConnState>,
    query: Arc<str>,
    /// Projected variables = value columns, in projection order.
    vars: Arc<[String]>,
    /// Validated `key=value` arguments of CREATE VIRTUAL TABLE.
    overrides: Arc<[(Key, String)]>,
}

/// What a declared column is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Column {
    Value(usize),
    Term(usize),
    Id(usize),
    Opts,
}

fn column(n: usize, i: usize) -> Option<Column> {
    match i {
        _ if i < n => Some(Column::Value(i)),
        _ if i < 2 * n => Some(Column::Term(i - n)),
        _ if i < 3 * n => Some(Column::Id(i - 2 * n)),
        _ if i == 3 * n => Some(Column::Opts),
        _ => None,
    }
}

/// The projected variables of a SELECT query (spargebra's projection order; `SELECT *` gives
/// the in-scope variables).
fn select_variables(q: &spargebra::Query) -> Result<Vec<String>> {
    use spargebra::algebra::GraphPattern;
    let spargebra::Query::Select { pattern, .. } = q else {
        return Err(Error::Invalid("sparql_view needs a SELECT query".into()));
    };
    let mut p = pattern;
    loop {
        match p {
            GraphPattern::Slice { inner, .. } | GraphPattern::Distinct { inner } | GraphPattern::Reduced { inner } => {
                p = inner
            }
            GraphPattern::Project { variables, .. } => {
                return Ok(variables.iter().map(|v| v.as_str().to_string()).collect());
            }
            _ => return Err(Error::internal("SELECT query without a projection")),
        }
    }
}

/// Splits the module arguments: the query (dequoted) and validated `key=value` settings.
fn parse_arguments(args: &[&[u8]]) -> Result<(String, Vec<(Key, String)>)> {
    let Some((first, rest)) = args.split_first() else {
        return Err(Error::Invalid("sparql_view needs the SPARQL query as its first argument".into()));
    };
    let text = std::str::from_utf8(first).map_err(|_| Error::Invalid("sparql_view: the query is not UTF-8".into()))?;
    let query = rusqlite::vtab::dequote(text.trim()).into_owned();
    let mut overrides = Vec::with_capacity(rest.len());
    let mut check = QuerySettings::default();
    for arg in rest {
        let (k, v) = parameter(arg).map_err(|e| Error::Invalid(format!("sparql_view: {e}")))?;
        let key = Key::parse(k)?;
        if !key.per_call() {
            return Err(Error::Invalid(format!("sparql_view: '{}' is a connection setting", key.name())));
        }
        check.set(key, &v)?;
        overrides.push((key, v.into_owned()));
    }
    Ok((query, overrides))
}

/// The declared schema; fails on column-name clashes (SQLite names are case-insensitive).
fn schema(vars: &[String]) -> Result<CString> {
    let mut names: Vec<String> = Vec::with_capacity(3 * vars.len() + 1);
    names.extend(vars.iter().cloned());
    names.extend(vars.iter().map(|v| format!("{v}__term")));
    names.extend(vars.iter().map(|v| format!("{v}__id")));
    names.push("opts".into());
    let mut seen = rustc_hash::FxHashSet::default();
    for n in &names {
        if !seen.insert(n.to_ascii_lowercase()) {
            return Err(Error::Invalid(format!("sparql_view: column name '{n}' is used twice (rename the variable)")));
        }
    }
    let n = vars.len();
    let mut sql = String::from("CREATE TABLE x(");
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            sql.push(',');
        }
        sql.push('"');
        sql.push_str(&escape_double_quote(name));
        sql.push('"');
        if i >= n {
            sql.push_str(" HIDDEN");
        }
    }
    sql.push(')');
    CString::new(sql).map_err(|_| Error::Invalid("sparql_view: a variable name contains NUL".into()))
}

unsafe impl<'vtab> VTab<'vtab> for ViewTab {
    type Aux = Arc<ConnState>;
    type Cursor = ViewCursor;

    fn connect(
        _db: &mut VTabConnection,
        aux: Option<&Arc<ConnState>>,
        _module: &[u8],
        _database: &[u8],
        _table: &[u8],
        args: &[&[u8]],
    ) -> rusqlite::Result<(Cow<'static, CStr>, Self)> {
        guard("sparql_view xConnect", || {
            let st =
                aux.cloned().ok_or_else(|| rusqlite::Error::ModuleError("sparql_view: no connection state".into()))?;
            // Only parsing here: the dictionary is consulted when the view is scanned.
            let (query, overrides) = parse_arguments(args).into_sql()?;
            let parsed = crate::nesting::parse_query(&query, None).into_sql()?;
            let vars = select_variables(&parsed).into_sql()?;
            let sql = schema(&vars).into_sql()?;
            let tab = ViewTab {
                base: sqlite3_vtab::default(),
                st,
                query: query.into(),
                vars: vars.into(),
                overrides: overrides.into(),
            };
            Ok((Cow::Owned(sql), tab))
        })
    }

    fn best_index(&self, info: &mut IndexInfo) -> rusqlite::Result<bool> {
        guard("sparql_view xBestIndex", || {
            self.st.debug_panic(PanicSite::BestIndex);
            let n = self.vars.len();
            let mut consumed: Vec<(usize, c_int)> = Vec::new();
            let mut opts_taken = false;
            for (i, c) in info.constraints().enumerate() {
                if !c.is_usable() || c.operator() != IndexConstraintOp::SQLITE_INDEX_CONSTRAINT_EQ {
                    continue;
                }
                match column(n, c.column() as usize) {
                    // `sameTerm` is the BINARY comparison of the N-Triples text; SQLite filters
                    // other collations itself.
                    Some(Column::Term(_)) if info.collation(i).is_ok_and(|c| c.eq_ignore_ascii_case("BINARY")) => {
                        consumed.push((i, c.column()))
                    }
                    // INTEGER comparisons: no collation applies.
                    Some(Column::Id(_)) => consumed.push((i, c.column())),
                    Some(Column::Opts) if !opts_taken => {
                        opts_taken = true;
                        consumed.push((i, c.column()));
                    }
                    _ => {}
                }
            }
            let selective =
                consumed.iter().any(|(_, c)| matches!(column(n, *c as usize), Some(Column::Term(_) | Column::Id(_))));
            let mut idx = format!("{:x}:", info.col_used());
            for (argv, (i, col)) in consumed.iter().enumerate() {
                let mut u = info.constraint_usage(*i);
                u.set_argv_index(argv as c_int + 1);
                u.set_omit(true);
                if argv > 0 {
                    idx.push(',');
                }
                idx.push_str(&col.to_string());
            }
            info.set_idx_str(&idx);
            let rows = if selective { 10 } else { 1000 };
            info.set_estimated_rows(rows);
            info.set_estimated_cost(rows as f64);
            Ok(true)
        })
    }

    fn open(&'vtab mut self) -> rusqlite::Result<ViewCursor> {
        guard("sparql_view xOpen", || {
            self.st.debug_panic(PanicSite::Open);
            Ok(ViewCursor {
                base: sqlite3_vtab_cursor::default(),
                st: self.st.clone(),
                query: self.query.clone(),
                vars: self.vars.clone(),
                overrides: self.overrides.clone(),
                scan: RefCell::new(None),
                text: RefCell::new(String::new()),
            })
        })
    }
}

impl CreateVTab<'_> for ViewTab {
    const KIND: VTabKind = VTabKind::Default;
}

/// One scan of the view.
struct Scan {
    // Drop order: execution, then the open-cursor count.
    stream: Stream,
    /// View value column → stream column.
    map: Vec<Option<usize>>,
    opts: Option<String>,
    _token: CursorToken,
}

#[repr(C)]
pub(crate) struct ViewCursor {
    /// Base class; must be first.
    base: sqlite3_vtab_cursor,
    st: Arc<ConnState>,
    query: Arc<str>,
    vars: Arc<[String]>,
    overrides: Arc<[(Key, String)]>,
    scan: RefCell<Option<Scan>>,
    text: RefCell<String>,
}

impl Drop for ViewCursor {
    fn drop(&mut self) {
        let scan = self.scan.get_mut().take();
        guard_drop(|| {
            self.st.debug_panic(PanicSite::Close);
            drop(scan);
        });
    }
}

/// A pushed-down constraint value, resolved to a constant id (None = no row can match).
fn constraint_id(
    store: &sq_sqlite::SqliteStore,
    col: Column,
    value: ValueRef<'_>,
    temps: &mut sq_core::eval_api::TempDict,
) -> Result<Option<TermId>> {
    match col {
        Column::Term(_) => {
            // The column holds canonical N-Triples text: other spellings of the same term
            // (`@EN`, `^^xsd:string`, escapes) never compare equal in SQL, so they match
            // nothing here either.
            let ValueRef::Text(bytes) = value else { return Ok(None) };
            let Ok(text) = std::str::from_utf8(bytes) else { return Ok(None) };
            let Ok(parsed) = parse_term(text) else { return Ok(None) };
            let term = Term::from_oxrdf(parsed.as_ref());
            let mut canonical = String::with_capacity(text.len());
            if write_ntriples(&term, &mut canonical).is_err() || canonical != text {
                return Ok(None);
            }
            match store.lookup(parsed.as_ref())? {
                Some(id) => Ok(Some(id)),
                // Absent from the store: only a computed value can equal it (TEMP constant).
                None => Ok(Some(temps.intern(term))),
            }
        }
        Column::Id(_) => {
            let v = match value {
                ValueRef::Integer(v) => v,
                // SQL compares INTEGER and REAL numerically: an integral REAL names that id.
                ValueRef::Real(f) => match integral(f) {
                    Some(v) => v,
                    None => return Ok(None),
                },
                _ => return Ok(None),
            };
            let id = TermId(v);
            if !id.is_storable() || (v < 0 && !id.is_inline()) {
                return Ok(None);
            }
            let valid = if id.is_inline() {
                codec::decode_inline(id).and_then(|t| codec::encode_inline(term_ref(&t))) == Some(id)
            } else if id.tag() == Tag::BNode {
                true
            } else {
                // A dictionary id must name an existing row (constants are decoded by the
                // evaluator's TEMP comparisons).
                match store.decode_one(id) {
                    Ok(_) => true,
                    Err(Error::Internal(_)) => false,
                    Err(e) => return Err(e),
                }
            };
            Ok(valid.then_some(id))
        }
        Column::Value(_) | Column::Opts => Err(Error::internal("not a pushed-down column")),
    }
}

/// The i64 equal to `f`, if any (SQLite compares INTEGER and REAL values exactly).
fn integral(f: f64) -> Option<i64> {
    // 2^63 is exactly representable; every integral f64 in [-2^63, 2^63) converts exactly.
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    (f.fract() == 0.0 && (-LIMIT..LIMIT).contains(&f)).then_some(f as i64)
}

fn term_ref(t: &Term) -> oxrdf::TermRef<'_> {
    crate::helpers::term_ref(t)
}

impl ViewCursor {
    /// Plans the scan and fetches its first batch, as one read entry.
    fn open_scan(&self, idx_str: Option<&str>, args: &Filters<'_>) -> Result<Scan> {
        self.st.read_call(|| self.start_scan(idx_str, args))
    }

    fn start_scan(&self, idx_str: Option<&str>, args: &Filters<'_>) -> Result<Scan> {
        let st = &self.st;
        let n = self.vars.len();
        let (used, cols) = parse_idx(idx_str)?;
        let values: Vec<ValueRef<'_>> = args.iter().collect();
        let value = |j: usize| values.get(j).copied().unwrap_or(ValueRef::Null);

        let conn = st.settings()?;
        let mut settings = conn.clone();
        for (k, v) in self.overrides.iter() {
            settings.set(*k, v)?;
        }
        let mut opts_arg = None;
        let mut empty = false;
        for (j, col) in cols.iter().enumerate() {
            if column(n, *col) == Some(Column::Opts) {
                // The column holds the TEXT given here: `opts = NULL` / `opts = 42` is never true.
                let ValueRef::Text(t) = value(j) else {
                    empty = true;
                    continue;
                };
                let json =
                    std::str::from_utf8(t).map_err(|_| Error::Invalid("sparql_view: opts is not UTF-8".into()))?;
                settings.apply_json(json)?;
                opts_arg = Some(json.to_string());
            }
        }
        // The definition's arguments and the `opts` column may only tighten the limits.
        settings.within_limits_of(&conn);
        let opts = settings.options();
        let parsed = st.parse_query(&self.query, opts.base_iri.as_deref())?;
        let mut lowered = sq_plan::lower_query(&parsed, st.store(), &opts)?;
        let QueryForm::Select(slots) = &lowered.form else {
            return Err(Error::internal("sparql_view query is not a SELECT"));
        };
        let slot_of = |var: &str| slots.iter().copied().find(|s| lowered.vars.name(*s) == var);

        // Pushed-down constraints → sameTerm filters (or an empty scan).
        let mut filters: Vec<Expr> = Vec::new();
        for (j, col) in cols.iter().enumerate() {
            let Some(c @ (Column::Term(k) | Column::Id(k))) = column(n, *col) else { continue };
            let Some(slot) = slot_of(&self.vars[k]) else {
                return Err(Error::internal("view variable without a slot"));
            };
            match constraint_id(st.store(), c, value(j), &mut lowered.temps)? {
                Some(id) => filters.push(Expr::SameTerm(Box::new(Expr::Slot(slot)), Box::new(Expr::Const(id)))),
                None => empty = true,
            }
        }

        let token = CursorToken::new(st);
        let (source, map, width) = if empty {
            (Source::Empty, vec![None; n], 0)
        } else {
            if let Some(expr) = filters.into_iter().reduce(|a, b| Expr::And(Box::new(a), Box::new(b))) {
                let root = std::mem::replace(&mut lowered.root, Logical::Unit);
                lowered.root = Logical::Filter { input: Box::new(root), expr };
            }
            let mut run = query::prepare_lowered(st, lowered, &opts)?;
            let out_vars = run.solutions().variables().to_vec();
            let map: Vec<Option<usize>> = self.vars.iter().map(|v| out_vars.iter().position(|o| o == v)).collect();
            (Source::Solutions(run), map, out_vars.len())
        };
        let mut decode: Vec<usize> =
            (0..n).filter(|&k| col_used(used, k) || col_used(used, n + k)).filter_map(|k| map[k]).collect();
        decode.sort_unstable();
        decode.dedup();
        let stream = Stream::open(st, source, width, decode)?;
        Ok(Scan { stream, map, opts: opts_arg, _token: token })
    }
}

/// `idx_str` = `<colUsed hex>:<col>,<col>,…` (the columns of the argv values, in order).
fn parse_idx(idx_str: Option<&str>) -> Result<(u64, Vec<usize>)> {
    let s = idx_str.unwrap_or("");
    let (used, cols) = s.split_once(':').unwrap_or((s, ""));
    let used = u64::from_str_radix(used, 16).unwrap_or(u64::MAX);
    let cols = cols
        .split(',')
        .filter(|c| !c.is_empty())
        .map(|c| c.parse::<usize>().map_err(|_| Error::internal(format!("bad sparql_view idx_str {s:?}"))))
        .collect::<Result<_>>()?;
    Ok((used, cols))
}

impl ViewCursor {
    /// The `xColumn` body; its error goes through [`column_error`].
    fn column_value(&self, ctx: &mut Context, i: c_int) -> rusqlite::Result<()> {
        let mut scan = self.scan.try_borrow_mut().map_err(|_| sql_error(Error::Reentrant))?;
        let Some(scan) = scan.as_mut() else { return ctx.set_result(&Null) };
        let n = self.vars.len();
        let st = &*self.st;
        match column(n, i as usize) {
            Some(Column::Value(k)) => {
                let term = match scan.map[k] {
                    Some(c) => scan.stream.term(st, c).into_sql()?,
                    None => None,
                };
                ctx.set_result(&native(term.as_deref()))
            }
            Some(Column::Term(k)) => {
                let term = match scan.map[k] {
                    Some(c) => scan.stream.term(st, c).into_sql()?,
                    None => None,
                };
                match term {
                    Some(t) => {
                        let mut text = self.text.borrow_mut();
                        text.clear();
                        write_ntriples(&t, &mut text).into_sql()?;
                        ctx.set_result(&text.as_str())
                    }
                    None => ctx.set_result(&Null),
                }
            }
            Some(Column::Id(k)) => {
                let id = match scan.map[k] {
                    Some(c) => scan.stream.public_id(st, c).into_sql()?,
                    None => None,
                };
                ctx.set_result(&id)
            }
            Some(Column::Opts) => ctx.set_result(&scan.opts.as_deref()),
            None => ctx.set_result(&Null),
        }
    }
}

unsafe impl VTabCursor for ViewCursor {
    fn filter(&mut self, _idx_num: c_int, idx_str: Option<&str>, args: &Filters<'_>) -> rusqlite::Result<()> {
        guard("sparql_view xFilter", || {
            self.st.debug_panic(PanicSite::Filter);
            *self.scan.get_mut() = None;
            let scan = self.open_scan(idx_str, args).into_sql()?;
            *self.scan.get_mut() = Some(scan);
            Ok(())
        })
    }

    fn next(&mut self) -> rusqlite::Result<()> {
        guard("sparql_view xNext", || {
            self.st.debug_panic(PanicSite::Next);
            match self.scan.get_mut() {
                Some(scan) => {
                    let _step = self.st.step_entry().into_sql()?;
                    scan.stream.advance(&self.st).into_sql()
                }
                None => Ok(()),
            }
        })
    }

    fn eof(&self) -> bool {
        guard_eof(|| {
            self.st.debug_panic(PanicSite::Eof);
            self.scan.try_borrow().map_or(true, |s| s.as_ref().is_none_or(|s| s.stream.eof()))
        })
    }

    fn column(&self, ctx: &mut Context, i: c_int) -> rusqlite::Result<()> {
        guard("sparql_view xColumn", || {
            self.st.debug_panic(PanicSite::Column);
            let r = self.st.step_entry().into_sql().and_then(|_step| self.column_value(ctx, i));
            column_error(ctx, r)
        })
    }

    fn rowid(&self) -> rusqlite::Result<i64> {
        guard("sparql_view xRowid", || {
            self.st.debug_panic(PanicSite::Rowid);
            Ok(self.scan.try_borrow().ok().and_then(|s| s.as_ref().map(|s| s.stream.ordinal())).unwrap_or(0))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_laid_out_in_three_groups_plus_opts() {
        assert_eq!(column(2, 0), Some(Column::Value(0)));
        assert_eq!(column(2, 3), Some(Column::Term(1)));
        assert_eq!(column(2, 4), Some(Column::Id(0)));
        assert_eq!(column(2, 6), Some(Column::Opts));
        assert_eq!(column(2, 7), None);
    }

    #[test]
    fn schema_quotes_names_and_detects_clashes() {
        let s = schema(&["a".into(), "b\"c".into()]).unwrap();
        assert_eq!(
            s.to_str().unwrap(),
            r#"CREATE TABLE x("a","b""c","a__term" HIDDEN,"b""c__term" HIDDEN,"a__id" HIDDEN,"b""c__id" HIDDEN,"opts" HIDDEN)"#
        );
        assert!(schema(&["x".into(), "x__term".into()]).is_err());
        assert!(schema(&["opts".into()]).is_err());
        assert!(schema(&["A".into(), "a__ID".into()]).is_err(), "A__id and a__ID clash case-insensitively");
    }

    #[test]
    fn arguments_and_projection() {
        let (q, o) =
            parse_arguments(&[b"'SELECT ?a WHERE { ?a ?b ''x'' }'", b"engine=sql", b" timeout_ms = 5 "]).unwrap();
        assert_eq!(q, "SELECT ?a WHERE { ?a ?b 'x' }");
        assert_eq!(o, vec![(Key::Engine, "sql".into()), (Key::TimeoutMs, "5".into())]);
        assert!(parse_arguments(&[b"'q'", b"allow_files=1"]).is_err());
        assert!(parse_arguments(&[b"'q'", b"engine=fast"]).is_err());
        assert!(parse_arguments(&[]).is_err());
        let star = sq_plan::parse_query("SELECT DISTINCT * WHERE { ?s ?p ?o } LIMIT 3", None).unwrap();
        assert_eq!(select_variables(&star).unwrap(), ["o", "p", "s"]);
        let ask = sq_plan::parse_query("ASK {}", None).unwrap();
        assert!(select_variables(&ask).is_err());
        assert_eq!(parse_idx(Some("ff:3,5")).unwrap(), (255, vec![3, 5]));
        assert_eq!(parse_idx(Some("1:")).unwrap(), (1, vec![]));
    }

    #[test]
    fn integral_reals_name_integers() {
        assert_eq!(integral(42.0), Some(42));
        assert_eq!(integral(-7.0), Some(-7));
        assert_eq!(integral(-9_223_372_036_854_775_808.0), Some(i64::MIN));
        assert_eq!(integral(9_223_372_036_854_775_808.0), None, "2^63 is outside i64");
        assert_eq!(integral(4.5), None);
        assert_eq!(integral(f64::NAN), None);
        assert_eq!(integral(f64::INFINITY), None);
    }
}
