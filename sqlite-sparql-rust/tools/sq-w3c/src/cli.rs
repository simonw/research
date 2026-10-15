//! Command-line options.
use std::path::PathBuf;
use std::time::Duration;

use crate::backend::BackendKind;

pub const USAGE: &str = "\
usage: sq-w3c --suite <dir> --backend <reference|mem|sqlite|oracle> [options]

  --suite <dir>             the `sparql` directory of a w3c/rdf-tests checkout
  --backend <name>          engine under test: reference (spareval, validates the harness),
                            mem (MemStore + sq-engine), sqlite (in-memory SQLite + sq-api),
                            oracle (sq-oracle, queries only)
  --filter <text>           run only tests whose IRI, name or manifest contains <text>
                            (repeatable: any match)
  --manifest <path>         entry manifest relative to the suite (repeatable; default:
                            sparql11/manifest-all.ttl and sparql10/manifest-evaluation.ttl)
  --known-failures <file>   test IRIs expected to fail (one per line, # comments)
  --report <file>           write the JSON report there
  --timeout <seconds>       per-test timeout (default 30)
  --verbose                 print every non-passing test with its full message
  -h, --help                this text

Exit status: 0 = no unexpected failures, 1 = a failing test is not in --known-failures,
2 = usage or harness error.";

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub suite: PathBuf,
    pub backend: BackendKind,
    pub filters: Vec<String>,
    pub manifests: Vec<String>,
    pub known_failures: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub timeout: Duration,
    pub verbose: bool,
}

/// What the command line asks for.
#[derive(Debug, PartialEq)]
pub enum Command {
    Run(Options),
    Help,
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let mut suite = None;
    let mut backend = None;
    let mut options = Options {
        suite: PathBuf::new(),
        backend: BackendKind::Reference,
        filters: Vec::new(),
        manifests: Vec::new(),
        known_failures: None,
        report: None,
        timeout: Duration::from_secs(30),
        verbose: false,
    };
    while let Some(arg) = args.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            _ => (arg.clone(), None),
        };
        let mut value = || inline.clone().or_else(|| args.next()).ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "--suite" => suite = Some(PathBuf::from(value()?)),
            "--backend" => backend = Some(BackendKind::parse(&value()?)?),
            "--filter" => options.filters.push(value()?),
            "--manifest" => options.manifests.push(value()?),
            "--known-failures" => options.known_failures = Some(PathBuf::from(value()?)),
            "--report" => options.report = Some(PathBuf::from(value()?)),
            "--timeout" => {
                let v = value()?;
                let secs: f64 = v.parse().map_err(|_| format!("--timeout: not a number: {v}"))?;
                if !(secs.is_finite() && secs > 0.0) {
                    return Err(format!("--timeout must be positive, got {v}"));
                }
                options.timeout = Duration::from_secs_f64(secs);
            }
            "--verbose" | "-v" => options.verbose = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    options.suite = suite.ok_or("--suite is required")?;
    options.backend = backend.ok_or("--backend is required")?;
    Ok(Command::Run(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_all_options() {
        let Command::Run(o) = parse(&[
            "--suite", "/s", "--backend", "SQLite", "--filter", "agg", "--filter=bind", "--manifest",
            "sparql11/manifest-sparql11-query.ttl", "--known-failures", "kf.txt", "--report", "r.json",
            "--timeout", "2.5", "--verbose",
        ])
        .unwrap() else {
            panic!()
        };
        assert_eq!(o.suite, PathBuf::from("/s"));
        assert_eq!(o.backend, BackendKind::Sqlite);
        assert_eq!(o.filters, ["agg", "bind"]);
        assert_eq!(o.manifests, ["sparql11/manifest-sparql11-query.ttl"]);
        assert_eq!(o.known_failures, Some(PathBuf::from("kf.txt")));
        assert_eq!(o.report, Some(PathBuf::from("r.json")));
        assert_eq!(o.timeout, Duration::from_millis(2500));
        assert!(o.verbose);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(&["--backend", "mem"]).unwrap_err().contains("--suite"));
        assert!(parse(&["--suite", "/s"]).unwrap_err().contains("--backend"));
        assert!(parse(&["--suite", "/s", "--backend", "pg"]).is_err());
        assert!(parse(&["--suite"]).unwrap_err().contains("needs a value"));
        assert!(parse(&["--suite", "/s", "--backend", "mem", "--timeout", "0"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
        assert_eq!(parse(&["--help"]).unwrap(), Command::Help);
    }
}
