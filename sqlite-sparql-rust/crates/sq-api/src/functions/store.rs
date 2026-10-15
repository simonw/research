//! Store functions: version, init, loads, update, analyze, vacuum, check and configuration.
use std::path::Path;

use oxrdfio::RdfFormat;
use rusqlite::functions::Context;
use rusqlite::types::ValueRef;
use sq_core::{Error, Result};
use sq_engine::UpdateOptions;
use sq_sqlite::{LoadOptions, LoadReport, LoadSource};

use super::{opt_text_arg, setting_arg, text_arg};
use crate::config::Key;
use crate::state::{ChangeReport, ConnState};

/// `sparql_version()`: crate version and the SQLite library version.
pub(super) fn version(_st: &ConnState, _ctx: &Context<'_>) -> Result<String> {
    Ok(crate::version_string(rusqlite::version()))
}

/// `sparql_init()`: idempotent §2.1 DDL; returns the schema version.
pub(super) fn init(st: &ConnState, _ctx: &Context<'_>) -> Result<i64> {
    let v = st.writer_call(|| st.store().ensure_schema())?;
    st.record(ChangeReport { op: "init", ..ChangeReport::default() });
    Ok(v)
}

/// RDF format names of `sparql_load` (and the extensions `sparql_load_file` guesses from).
pub(crate) fn rdf_format(name: &str) -> Result<RdfFormat> {
    Ok(match name.trim().to_ascii_lowercase().as_str() {
        "turtle" | "ttl" => RdfFormat::Turtle,
        "ntriples" | "nt" => RdfFormat::NTriples,
        "nquads" | "nq" => RdfFormat::NQuads,
        "trig" => RdfFormat::TriG,
        "rdfxml" | "xml" | "rdf" | "owl" => RdfFormat::RdfXml,
        "n3" => RdfFormat::N3,
        "jsonld" | "json" => RdfFormat::JsonLd { profile: Default::default() },
        _ => {
            return Err(Error::Invalid(format!(
                "unknown RDF format '{name}' (expected turtle/ttl, ntriples/nt, nquads/nq, trig, rdfxml/xml, n3 or jsonld)"
            )));
        }
    })
}

fn load_options(ctx: &Context<'_>, format: RdfFormat) -> Result<LoadOptions> {
    let mut opts = LoadOptions::new(format);
    opts.base_iri = opt_text_arg(ctx, 2, "base")?.map(str::to_string);
    opts.graph = opt_text_arg(ctx, 3, "graph")?.map(str::to_string);
    Ok(opts)
}

fn run_load(st: &ConnState, src: LoadSource<'_>, opts: &LoadOptions) -> Result<i64> {
    let report: LoadReport = st.write_call(true, || st.store().bulk_load(src, opts))?;
    st.record(ChangeReport {
        op: "load",
        inserted: report.quads_new,
        quads_read: report.quads_read,
        terms_new: report.terms_new,
        bulk: report.bulk,
        ms: report.ms,
        ..ChangeReport::default()
    });
    Ok(report.quads_new as i64)
}

/// `sparql_load(data TEXT|BLOB [, format [, base [, graph]]])`: new quads.
pub(super) fn load(st: &ConnState, ctx: &Context<'_>) -> Result<i64> {
    let data = match ctx.get_raw(0) {
        ValueRef::Text(b) | ValueRef::Blob(b) => b,
        ValueRef::Null => return Err(Error::Invalid("sparql_load: data is NULL".into())),
        other => {
            return Err(Error::Invalid(format!("sparql_load: data must be TEXT or BLOB, got {}", other.data_type())));
        }
    };
    let format = match opt_text_arg(ctx, 1, "format")? {
        Some(f) => rdf_format(f)?,
        None => RdfFormat::Turtle,
    };
    run_load(st, LoadSource::Bytes(data), &load_options(ctx, format)?)
}

/// `sparql_load_file(path [, format [, base [, graph]]])`: needs `allow_files`; the format
/// defaults from the extension and the base IRI to the file's `file://` URL.
pub(super) fn load_file(st: &ConnState, ctx: &Context<'_>) -> Result<i64> {
    if !st.allow_files()? {
        return Err(Error::Invalid(
            "sparql_load_file is disabled; enable it with sparql_config('allow_files', 1)".into(),
        ));
    }
    let path = Path::new(text_arg(ctx, 0, "path")?);
    let format = match opt_text_arg(ctx, 1, "format")? {
        Some(f) => rdf_format(f)?,
        None => {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            rdf_format(ext).map_err(|_| {
                Error::Invalid(format!("sparql_load_file: cannot guess the format of {}; pass it", path.display()))
            })?
        }
    };
    let mut opts = load_options(ctx, format)?;
    if opts.base_iri.is_none() {
        opts.base_iri = file_url(path);
    }
    run_load(st, LoadSource::File(path), &opts)
}

/// `file://` URL of an existing path (percent-encoding everything outside the IRI-safe set).
fn file_url(path: &Path) -> Option<String> {
    let abs = std::fs::canonicalize(path).ok()?;
    let s = abs.to_str()?;
    let mut url = String::with_capacity(s.len() + 8);
    url.push_str("file://");
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~!$&'()*+,;=:@".contains(&b) {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    Some(url)
}

/// `sparql_update(update)`: one write scope per request; returns inserted + deleted.
pub(super) fn update(st: &ConnState, ctx: &Context<'_>) -> Result<i64> {
    let text = text_arg(ctx, 0, "update")?;
    let opts = st.settings()?.options();
    // Parsed before the write scope opens: no lock is held while parsing.
    let parsed = crate::nesting::parse_update(text, opts.base_iri.as_deref())?;
    let uopts = UpdateOptions { allow_files: st.allow_files()?, ..UpdateOptions::default() };
    let report = st.write_call(true, || sq_engine::update_with(st.writer(), &parsed, &opts, &uopts))?;
    st.record(ChangeReport {
        op: "update",
        inserted: report.inserted,
        deleted: report.deleted,
        ..ChangeReport::default()
    });
    Ok((report.inserted + report.deleted) as i64)
}

/// `sparql_analyze()`: recomputes the statistics; returns a JSON summary.
pub(super) fn analyze(st: &ConnState, _ctx: &Context<'_>) -> Result<String> {
    let start = std::time::Instant::now();
    let stats = st.write_call(false, || st.store().analyze())?;
    let ms = start.elapsed().as_millis() as u64;
    st.record(ChangeReport { op: "analyze", ms, ..ChangeReport::default() });
    let scope = |s: &sq_core::stats::ScopeStats| {
        serde_json::json!({
            "triples": s.n,
            "subjects": s.ds,
            "predicates": s.dp,
            "objects": s.dobj,
            "characteristic_sets": s.cs.len(),
            "heavy_hitters": s.po.len(),
        })
    };
    let summary = serde_json::json!({
        "analyzed_quads": stats.analyzed_quads,
        "default_graph": scope(&stats.scope[0]),
        "named_graphs": scope(&stats.scope[1]),
        "ms": ms,
    });
    Ok(summary.to_string())
}

/// `sparql_vacuum_terms()`: deletes unreferenced dictionary rows.
pub(super) fn vacuum_terms(st: &ConnState, _ctx: &Context<'_>) -> Result<i64> {
    let n = st.write_call(false, || st.store().vacuum_terms())?;
    st.record(ChangeReport { op: "vacuum_terms", deleted: n, ..ChangeReport::default() });
    Ok(n as i64)
}

/// `sparql_check()`: verifies the storage invariants.
pub(super) fn check(st: &ConnState, _ctx: &Context<'_>) -> Result<String> {
    st.read_call(|| st.store().check())
}

/// `sparql_config(key)`.
pub(super) fn config_get(st: &ConnState, ctx: &Context<'_>) -> Result<String> {
    st.config_get(Key::parse(text_arg(ctx, 0, "key")?)?)
}

/// `sparql_config(key, value)` (DIRECTONLY): sets and returns the new value.
pub(super) fn config_set(st: &ConnState, ctx: &Context<'_>) -> Result<String> {
    let key = Key::parse(text_arg(ctx, 0, "key")?)?;
    st.config_set(key, &setting_arg(ctx, 1)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_names() {
        for (name, f) in [
            ("turtle", RdfFormat::Turtle),
            ("TTL", RdfFormat::Turtle),
            ("nt", RdfFormat::NTriples),
            ("nquads", RdfFormat::NQuads),
            ("trig", RdfFormat::TriG),
            ("rdfxml", RdfFormat::RdfXml),
            ("xml", RdfFormat::RdfXml),
            ("n3", RdfFormat::N3),
        ] {
            assert_eq!(rdf_format(name).unwrap(), f, "{name}");
        }
        assert!(matches!(rdf_format("jsonld").unwrap(), RdfFormat::JsonLd { .. }));
        assert!(rdf_format("csv").is_err());
    }

    #[test]
    fn file_urls_are_percent_encoded() {
        let dir = std::env::temp_dir().join("sq api dir é");
        std::fs::create_dir_all(&dir).unwrap();
        let url = file_url(&dir).unwrap();
        assert!(url.starts_with("file:///") && url.contains("sq%20api%20dir%20%C3%A9"), "{url}");
        assert!(oxrdf::NamedNode::new(url).is_ok());
        let _ = std::fs::remove_dir(&dir);
        assert_eq!(file_url(Path::new("/definitely/not/here")), None);
    }
}
