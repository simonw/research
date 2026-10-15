//! Per-connection configuration (§8.5, `sparql_config`) and per-call overrides (the `opts`
//! argument of `sparql_rows`, the `key=value` arguments of `sparql_view`).
//!
//! Settings live in two places: query settings ([`QuerySettings`], turned into
//! `sq_core::plan::QueryOptions` for every call and overridable per call) and connection-wide
//! settings ([`ConnConfig`]: dictionary-cache budget, `allow_files`, the debugging hook). The
//! B-tree reader settings (`reader`, `page_cache_mb`) are owned by the storage layer and only
//! routed through here (`ConnState::set_config`).
use sq_core::plan::{EngineKind, JoinPolicy, QueryOptions};
use sq_core::{Error, Result};

/// Every `sparql_config` key.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    Engine,
    TimeoutMs,
    MaxMemoryMb,
    UnionDefaultGraph,
    DecodeCacheMb,
    JoinPolicy,
    Reorder,
    ProbeCounts,
    ProbeCap,
    AllowFiles,
    Reader,
    PageCacheMb,
    /// Testing hook: makes one FFI entry point panic (panic-safety tests). Available in debug
    /// builds, and in release builds only when `SPARQLITE_DEBUG_HOOKS=1` is set before the
    /// extension is loaded (see `ConnState::debug_panic`).
    DebugPanic,
}

impl Key {
    /// The documented keys, in `sparql_status` order (`debug_panic` is deliberately absent).
    pub(crate) const DOCUMENTED: [Key; 12] = [
        Key::Engine,
        Key::TimeoutMs,
        Key::MaxMemoryMb,
        Key::UnionDefaultGraph,
        Key::DecodeCacheMb,
        Key::JoinPolicy,
        Key::Reorder,
        Key::ProbeCounts,
        Key::ProbeCap,
        Key::AllowFiles,
        Key::Reader,
        Key::PageCacheMb,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Key::Engine => "engine",
            Key::TimeoutMs => "timeout_ms",
            Key::MaxMemoryMb => "max_memory_mb",
            Key::UnionDefaultGraph => "union_default_graph",
            Key::DecodeCacheMb => "decode_cache_mb",
            Key::JoinPolicy => "join_policy",
            Key::Reorder => "reorder",
            Key::ProbeCounts => "probe_counts",
            Key::ProbeCap => "probe_cap",
            Key::AllowFiles => "allow_files",
            Key::Reader => "reader",
            Key::PageCacheMb => "page_cache_mb",
            Key::DebugPanic => "debug_panic",
        }
    }

    pub(crate) fn parse(name: &str) -> Result<Key> {
        let name = name.trim();
        Key::DOCUMENTED
            .iter()
            .chain([Key::DebugPanic].iter())
            .copied()
            .find(|k| k.name().eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                let known: Vec<&str> = Key::DOCUMENTED.iter().map(|k| k.name()).collect();
                Error::Invalid(format!("unknown sparql_config key '{name}' (known: {})", known.join(", ")))
            })
    }

    /// Keys a single call may override (`sparql_rows` opts, `sparql_view` arguments).
    pub(crate) const fn per_call(self) -> bool {
        matches!(
            self,
            Key::Engine
                | Key::TimeoutMs
                | Key::MaxMemoryMb
                | Key::UnionDefaultGraph
                | Key::JoinPolicy
                | Key::Reorder
                | Key::ProbeCounts
                | Key::ProbeCap
        )
    }
}

/// Settings that shape one query execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QuerySettings {
    pub engine: EngineKind,
    /// 0 = no timeout. Wall-clock time from the start of the execution: for a `sparql_rows` /
    /// `sparql_view` scan from its `xFilter`, so time the consumer spends between rows counts
    /// too (both engines; sq-exec fixes its deadline when the execution starts).
    pub timeout_ms: u64,
    pub max_memory_mb: u64,
    pub union_default_graph: bool,
    pub join_policy: JoinPolicy,
    pub reorder: bool,
    pub probe_counts: u32,
    pub probe_cap: u64,
}

impl Default for QuerySettings {
    fn default() -> Self {
        let d = QueryOptions::default();
        QuerySettings {
            engine: d.engine,
            timeout_ms: d.timeout_ms.unwrap_or(0),
            max_memory_mb: d.max_memory_mb,
            union_default_graph: d.union_default_graph,
            join_policy: d.join_policy,
            reorder: d.reorder,
            probe_counts: d.max_probe_counts,
            probe_cap: d.probe_count_cap,
        }
    }
}

impl QuerySettings {
    pub(crate) fn options(&self) -> QueryOptions {
        QueryOptions {
            engine: self.engine,
            timeout_ms: (self.timeout_ms > 0).then_some(self.timeout_ms),
            max_memory_mb: self.max_memory_mb,
            union_default_graph: self.union_default_graph,
            join_policy: self.join_policy,
            reorder: self.reorder,
            base_iri: None,
            max_probe_counts: self.probe_counts,
            probe_count_cap: self.probe_cap,
        }
    }

    /// Sets a per-call key (see [`Key::per_call`]).
    pub(crate) fn set(&mut self, key: Key, value: &str) -> Result<()> {
        match key {
            Key::Engine => self.engine = parse_engine(value)?,
            Key::TimeoutMs => self.timeout_ms = parse_u64(key, value)?,
            Key::MaxMemoryMb => self.max_memory_mb = parse_positive(key, value)?,
            Key::UnionDefaultGraph => self.union_default_graph = parse_bool(key, value)?,
            Key::JoinPolicy => self.join_policy = parse_join_policy(value)?,
            Key::Reorder => self.reorder = parse_bool(key, value)?,
            Key::ProbeCounts => {
                self.probe_counts = u32::try_from(parse_u64(key, value)?)
                    .map_err(|_| Error::Invalid(format!("{} is too large", key.name())))?
            }
            Key::ProbeCap => self.probe_cap = parse_u64(key, value)?,
            other => {
                return Err(Error::Invalid(format!("'{}' is a connection setting; use sparql_config", other.name())));
            }
        }
        Ok(())
    }

    pub(crate) fn get(&self, key: Key) -> Option<String> {
        Some(match key {
            Key::Engine => engine_name(self.engine).to_string(),
            Key::TimeoutMs => self.timeout_ms.to_string(),
            Key::MaxMemoryMb => self.max_memory_mb.to_string(),
            Key::UnionDefaultGraph => bool_text(self.union_default_graph).to_string(),
            Key::JoinPolicy => join_policy_name(self.join_policy).to_string(),
            Key::Reorder => bool_text(self.reorder).to_string(),
            Key::ProbeCounts => self.probe_counts.to_string(),
            Key::ProbeCap => self.probe_cap.to_string(),
            _ => return None,
        })
    }

    /// Applies a JSON object of per-call overrides (`sparql_rows(query, opts)`).
    pub(crate) fn apply_json(&mut self, json: &str) -> Result<()> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| Error::Invalid(format!("opts is not valid JSON: {e}")))?;
        let serde_json::Value::Object(map) = value else {
            return Err(Error::Invalid("opts must be a JSON object".into()));
        };
        for (k, v) in &map {
            let key = Key::parse(k)?;
            if !key.per_call() {
                return Err(Error::Invalid(format!("'{}' cannot be set per call; use sparql_config", key.name())));
            }
            let text = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => bool_text(*b).to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                other => return Err(Error::Invalid(format!("opts.{k}: unsupported value {other}"))),
            };
            self.set(key, &text)?;
        }
        Ok(())
    }
}

/// An FFI entry point the `debug_panic` hook can make panic.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum PanicSite {
    Scalar,
    BestIndex,
    Open,
    Filter,
    Next,
    Eof,
    Column,
    Rowid,
    Close,
}

impl PanicSite {
    const ALL: [PanicSite; 9] = [
        PanicSite::Scalar,
        PanicSite::BestIndex,
        PanicSite::Open,
        PanicSite::Filter,
        PanicSite::Next,
        PanicSite::Eof,
        PanicSite::Column,
        PanicSite::Rowid,
        PanicSite::Close,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            PanicSite::Scalar => "scalar",
            PanicSite::BestIndex => "xbestindex",
            PanicSite::Open => "xopen",
            PanicSite::Filter => "xfilter",
            PanicSite::Next => "xnext",
            PanicSite::Eof => "xeof",
            PanicSite::Column => "xcolumn",
            PanicSite::Rowid => "xrowid",
            PanicSite::Close => "xclose",
        }
    }

    fn parse(s: &str) -> Result<Option<PanicSite>> {
        let s = s.trim();
        if s.is_empty() || s.eq_ignore_ascii_case("off") || s == "0" {
            return Ok(None);
        }
        PanicSite::ALL.iter().copied().find(|p| p.name().eq_ignore_ascii_case(s)).map(Some).ok_or_else(|| {
            let names: Vec<&str> = PanicSite::ALL.iter().map(|p| p.name()).collect();
            Error::Invalid(format!("debug_panic: unknown site '{s}' (off, {})", names.join(", ")))
        })
    }
}

/// Connection-wide settings (the query settings plus what only `sparql_config` may set).
#[derive(Clone, Debug)]
pub(crate) struct ConnConfig {
    pub query: QuerySettings,
    pub decode_cache_mb: u64,
    pub allow_files: bool,
    pub debug_panic: Option<PanicSite>,
}

impl Default for ConnConfig {
    fn default() -> Self {
        ConnConfig {
            query: QuerySettings::default(),
            decode_cache_mb: sq_sqlite::StorageConfig::default().decode_cache_mb,
            allow_files: false,
            debug_panic: None,
        }
    }
}

impl ConnConfig {
    /// Sets a key owned by this struct (storage-owned keys are handled by the caller).
    pub(crate) fn set(&mut self, key: Key, value: &str) -> Result<()> {
        match key {
            Key::DecodeCacheMb => self.decode_cache_mb = parse_u64(key, value)?,
            Key::AllowFiles => self.allow_files = parse_bool(key, value)?,
            Key::DebugPanic => self.debug_panic = PanicSite::parse(value)?,
            Key::Reader | Key::PageCacheMb => {
                return Err(Error::internal(format!("{} is a storage setting", key.name())));
            }
            k => self.query.set(k, value)?,
        }
        Ok(())
    }

    /// Current value of a key owned by this struct (None for storage-owned keys).
    pub(crate) fn get(&self, key: Key) -> Option<String> {
        match key {
            Key::DecodeCacheMb => Some(self.decode_cache_mb.to_string()),
            Key::AllowFiles => Some(bool_text(self.allow_files).to_string()),
            Key::DebugPanic => Some(self.debug_panic.map_or("off", PanicSite::name).to_string()),
            Key::Reader | Key::PageCacheMb => None,
            k => self.query.get(k),
        }
    }
}

// ------------------------------------------------------------------ value parsing

fn bool_text(b: bool) -> &'static str {
    if b { "1" } else { "0" }
}

pub(crate) fn parse_bool(key: Key, value: &str) -> Result<bool> {
    rusqlite::vtab::parse_boolean(value.trim())
        .ok_or_else(|| Error::Invalid(format!("{}: expected 0/1 (or true/false), got '{value}'", key.name())))
}

pub(crate) fn parse_u64(key: Key, value: &str) -> Result<u64> {
    let v = value.trim();
    // Integral REAL values (e.g. 100.0 from a SQL expression) are accepted.
    let v = v.strip_suffix(".0").unwrap_or(v);
    v.parse::<u64>()
        .map_err(|_| Error::Invalid(format!("{}: expected a non-negative integer, got '{value}'", key.name())))
}

fn parse_positive(key: Key, value: &str) -> Result<u64> {
    match parse_u64(key, value)? {
        0 => Err(Error::Invalid(format!("{} must be at least 1", key.name()))),
        n => Ok(n),
    }
}

fn parse_engine(value: &str) -> Result<EngineKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "native" => Ok(EngineKind::Native),
        "sql" => Ok(EngineKind::Sql),
        _ => Err(Error::Invalid(format!("engine: expected native or sql, got '{value}'"))),
    }
}

pub(crate) const fn engine_name(e: EngineKind) -> &'static str {
    match e {
        EngineKind::Native => "native",
        EngineKind::Sql => "sql",
    }
}

fn parse_join_policy(value: &str) -> Result<JoinPolicy> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(JoinPolicy::Auto),
        "hash" => Ok(JoinPolicy::PreferHash),
        "bind" => Ok(JoinPolicy::PreferBind),
        "merge" => Ok(JoinPolicy::PreferMerge),
        "nosql" => Ok(JoinPolicy::NoSql),
        _ => Err(Error::Invalid(format!("join_policy: expected auto, hash, bind, merge or nosql, got '{value}'"))),
    }
}

const fn join_policy_name(p: JoinPolicy) -> &'static str {
    match p {
        JoinPolicy::Auto => "auto",
        JoinPolicy::PreferHash => "hash",
        JoinPolicy::PreferBind => "bind",
        JoinPolicy::PreferMerge => "merge",
        JoinPolicy::NoSql => "nosql",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec_table() {
        let c = ConnConfig::default();
        let get = |k| c.get(k).unwrap();
        assert_eq!(get(Key::Engine), "native");
        assert_eq!(get(Key::TimeoutMs), "0");
        assert_eq!(get(Key::MaxMemoryMb), "1024");
        assert_eq!(get(Key::UnionDefaultGraph), "0");
        assert_eq!(get(Key::DecodeCacheMb), "64");
        assert_eq!(get(Key::JoinPolicy), "auto");
        assert_eq!(get(Key::Reorder), "1");
        assert_eq!(get(Key::ProbeCounts), "32");
        assert_eq!(get(Key::ProbeCap), "1000");
        assert_eq!(get(Key::AllowFiles), "0");
        assert_eq!(get(Key::DebugPanic), "off");
        assert_eq!(c.get(Key::Reader), None, "storage-owned");
        assert_eq!(c.query.options().timeout_ms, None);
    }

    #[test]
    fn keys_parse_case_insensitively_and_reject_unknown_ones() {
        for k in Key::DOCUMENTED {
            assert_eq!(Key::parse(&k.name().to_uppercase()).unwrap(), k);
        }
        assert_eq!(Key::parse(" debug_panic ").unwrap(), Key::DebugPanic);
        let e = Key::parse("nope").unwrap_err().to_string();
        assert!(e.contains("unknown sparql_config key 'nope'") && e.contains("join_policy"), "{e}");
    }

    #[test]
    fn values_are_validated() {
        let mut c = ConnConfig::default();
        c.set(Key::Engine, "SQL").unwrap();
        c.set(Key::TimeoutMs, "250").unwrap();
        c.set(Key::Reorder, "false").unwrap();
        c.set(Key::JoinPolicy, "bind").unwrap();
        c.set(Key::AllowFiles, "1").unwrap();
        c.set(Key::MaxMemoryMb, "8.0").unwrap();
        let o = c.query.options();
        assert_eq!(o.engine, EngineKind::Sql);
        assert_eq!(o.timeout_ms, Some(250));
        assert!(!o.reorder);
        assert_eq!(o.join_policy, JoinPolicy::PreferBind);
        assert_eq!(o.max_memory_mb, 8);
        assert!(c.allow_files);
        for (k, v) in [
            (Key::Engine, "fast"),
            (Key::TimeoutMs, "-1"),
            (Key::Reorder, "maybe"),
            (Key::MaxMemoryMb, "0"),
            (Key::JoinPolicy, "loop"),
            (Key::DebugPanic, "everywhere"),
        ] {
            assert!(c.set(k, v).is_err(), "{k:?}={v}");
        }
        c.set(Key::DebugPanic, "xFilter").unwrap();
        assert_eq!(c.debug_panic, Some(PanicSite::Filter));
        c.set(Key::DebugPanic, "off").unwrap();
        assert_eq!(c.debug_panic, None);
    }

    #[test]
    fn json_overrides_only_touch_per_call_keys() {
        let mut q = QuerySettings::default();
        q.apply_json(r#"{"engine":"sql","timeout_ms":100,"reorder":false,"union_default_graph":true}"#).unwrap();
        assert_eq!(q.engine, EngineKind::Sql);
        assert_eq!(q.timeout_ms, 100);
        assert!(!q.reorder && q.union_default_graph);
        assert!(q.apply_json(r#"{"allow_files":1}"#).is_err());
        assert!(q.apply_json(r#"{"decode_cache_mb":1}"#).is_err());
        assert!(q.apply_json("[1]").is_err());
        assert!(q.apply_json("{").is_err());
        assert!(q.apply_json(r#"{"timeout_ms":[1]}"#).is_err());
    }
}
