//! `sq-w3c` command line: runs the W3C SPARQL test suite against one backend.
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use sq_w3c::cli::{parse_args, Command, Options, USAGE};
use sq_w3c::manifest::{load_tests, DEFAULT_MANIFESTS};
use sq_w3c::report::{load_known_failures, Record, Report};
use sq_w3c::runner::{Runner, Status};
use sq_w3c::suite::Suite;

fn main() -> ExitCode {
    match parse_args(std::env::args().skip(1)) {
        Ok(Command::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Command::Run(options)) => match run(&options) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::from(1),
            Err(e) => {
                eprintln!("sq-w3c: {e}");
                ExitCode::from(2)
            }
        },
        Err(e) => {
            eprintln!("sq-w3c: {e}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Runs the suite; Ok(true) when every failure is a known one.
fn run(options: &Options) -> Result<bool, String> {
    let start = Instant::now();
    let suite = Suite::new(&options.suite)?;
    let manifests: Vec<String> = if options.manifests.is_empty() {
        DEFAULT_MANIFESTS.iter().map(|m| suite.url(m)).collect()
    } else {
        options.manifests.iter().map(|m| suite.url(m)).collect()
    };
    let mut known = options.backend.builtin_known_failures();
    if let Some(path) = &options.known_failures {
        known.extend(load_known_failures(path)?);
    }
    let tests: Vec<Arc<_>> = load_tests(&suite, &manifests)?
        .into_iter()
        .filter(|t| {
            options.filters.is_empty()
                || options.filters.iter().any(|f| t.id.contains(f) || t.name.contains(f) || t.manifest.contains(f))
        })
        .map(Arc::new)
        .collect();
    if tests.is_empty() {
        return Err("no test matches".into());
    }

    let mut runner = Runner::new(options.backend, suite.clone(), options.timeout, options.verbose)?;
    let mut records = Vec::with_capacity(tests.len());
    for test in tests {
        let outcome = runner.run(&test);
        let known_failure = known.contains(&test.id);
        if options.verbose && outcome.status != Status::Pass {
            println!(
                "{:<12} {} ({}){}{}",
                outcome.status.name(),
                test.id,
                test.name,
                if known_failure { " [known failure]" } else { "" },
                outcome.message.as_deref().map(|m| format!("\n    {}", m.replace('\n', "\n    "))).unwrap_or_default()
            );
        }
        records.push(Record { test, outcome, known_failure });
    }

    let report = Report {
        backend: options.backend.name().to_owned(),
        suite: suite.root().display().to_string(),
        elapsed: start.elapsed(),
        records,
    };
    println!("{}", report.summary_markdown());

    let unexpected: Vec<&Record> = report.unexpected_failures().collect();
    if !unexpected.is_empty() {
        println!("Unexpected failures ({}):", unexpected.len());
        for r in &unexpected {
            let first_line = r.outcome.message.as_deref().and_then(|m| m.lines().next()).unwrap_or("");
            println!("- [{}] {} ({}): {first_line}", r.outcome.status.name(), r.test.id, r.test.name);
        }
    }
    let fixed: Vec<&Record> = report.fixed_known_failures().collect();
    if !fixed.is_empty() {
        println!("\nKnown failures that now pass ({}; remove them from the list):", fixed.len());
        for r in &fixed {
            println!("- {}", r.test.id);
        }
    }
    if let Some(path) = &options.report {
        let json = serde_json::to_string_pretty(&report.to_json()).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("\nJSON report: {}", path.display());
    }
    Ok(unexpected.is_empty())
}
