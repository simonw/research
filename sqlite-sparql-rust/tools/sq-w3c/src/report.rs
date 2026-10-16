//! Aggregation and output: the per-manifest summary table, the JSON report and the
//! known-failures list.
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::manifest::{TestCase, TestKind};
use crate::runner::{Outcome, Status};

/// Test IRIs listed in a known-failures file: one IRI per line; lines starting with `#` and
/// anything after the IRI (e.g. `  # reason`) are comments.
pub fn load_known_failures(path: &Path) -> Result<HashSet<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(parse_known_failures(&text))
}

pub fn parse_known_failures(text: &str) -> HashSet<String> {
    text.lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|iri| !iri.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub pass: usize,
    pub pass_lenient: usize,
    pub fail: usize,
    pub error: usize,
    pub skipped: usize,
}

impl Counts {
    fn add(&mut self, status: Status) {
        match status {
            Status::Pass => self.pass += 1,
            Status::PassLenient => self.pass_lenient += 1,
            Status::Fail => self.fail += 1,
            Status::Error => self.error += 1,
            Status::Skipped => self.skipped += 1,
        }
    }

    pub fn total(&self) -> usize {
        self.pass + self.pass_lenient + self.fail + self.error + self.skipped
    }

    /// Tests that ran (not skipped).
    pub fn run(&self) -> usize {
        self.total() - self.skipped
    }

    /// (pass + pass-lenient) / run, in percent.
    pub fn pass_rate(&self) -> f64 {
        if self.run() == 0 {
            return 100.0;
        }
        100.0 * (self.pass + self.pass_lenient) as f64 / self.run() as f64
    }

    fn to_json(self) -> Value {
        json!({
            "pass": self.pass,
            "pass_lenient": self.pass_lenient,
            "fail": self.fail,
            "error": self.error,
            "skipped": self.skipped,
            "total": self.total(),
        })
    }
}

pub struct Record {
    pub test: Arc<TestCase>,
    pub outcome: Outcome,
    pub known_failure: bool,
}

impl Record {
    /// A failure that is not in the known-failures list (fails the run).
    pub fn is_unexpected_failure(&self) -> bool {
        self.outcome.status.is_failure() && !self.known_failure
    }

    /// A known failure that now passes (the list should be updated).
    pub fn is_fixed(&self) -> bool {
        self.known_failure && matches!(self.outcome.status, Status::Pass | Status::PassLenient)
    }
}

pub struct Report {
    pub backend: String,
    pub suite: String,
    pub elapsed: Duration,
    pub records: Vec<Record>,
}

impl Report {
    pub fn totals(&self) -> Counts {
        let mut c = Counts::default();
        self.records.iter().for_each(|r| c.add(r.outcome.status));
        c
    }

    /// Counts per manifest, in first-seen order.
    pub fn per_manifest(&self) -> Vec<(String, Counts)> {
        let mut out: Vec<(String, Counts)> = Vec::new();
        for r in &self.records {
            let i = match out.iter().position(|(m, _)| *m == r.test.manifest) {
                Some(i) => i,
                None => {
                    out.push((r.test.manifest.clone(), Counts::default()));
                    out.len() - 1
                }
            };
            out[i].1.add(r.outcome.status);
        }
        out
    }

    /// Skip reasons with their counts, most frequent first.
    pub fn skip_reasons(&self) -> Vec<(String, usize)> {
        let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
        for r in self.records.iter().filter(|r| r.outcome.status == Status::Skipped) {
            *reasons.entry(r.outcome.message.clone().unwrap_or_default()).or_default() += 1;
        }
        let mut reasons: Vec<_> = reasons.into_iter().collect();
        reasons.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        reasons
    }

    pub fn unexpected_failures(&self) -> impl Iterator<Item = &Record> {
        self.records.iter().filter(|r| r.is_unexpected_failure())
    }

    pub fn fixed_known_failures(&self) -> impl Iterator<Item = &Record> {
        self.records.iter().filter(|r| r.is_fixed())
    }

    /// Markdown summary: per-manifest table, totals and skip reasons.
    pub fn summary_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "## W3C SPARQL test suite: backend `{}`\n", self.backend);
        let _ = writeln!(out, "| manifest | pass | pass-lenient | fail | error | skipped | total |");
        let _ = writeln!(out, "|---|---:|---:|---:|---:|---:|---:|");
        let row = |out: &mut String, name: &str, c: &Counts| {
            let _ = writeln!(
                out,
                "| {name} | {} | {} | {} | {} | {} | {} |",
                c.pass,
                c.pass_lenient,
                c.fail,
                c.error,
                c.skipped,
                c.total()
            );
        };
        for (manifest, counts) in self.per_manifest() {
            row(&mut out, &manifest, &counts);
        }
        let totals = self.totals();
        row(&mut out, "**total**", &totals);
        let _ = writeln!(
            out,
            "\n{} of {} tests run passed ({:.2}%; {} exactly, {} with numeric leniency); {} skipped. Elapsed {:.1} s.",
            totals.pass + totals.pass_lenient,
            totals.run(),
            totals.pass_rate(),
            totals.pass,
            totals.pass_lenient,
            totals.skipped,
            self.elapsed.as_secs_f64()
        );
        let known = self.records.iter().filter(|r| r.known_failure && r.outcome.status.is_failure()).count();
        if known > 0 {
            let _ = writeln!(out, "\n{known} failing tests are listed as known failures.");
        }
        let reasons = self.skip_reasons();
        if !reasons.is_empty() {
            let _ = writeln!(out, "\nSkipped:");
            for (reason, n) in reasons {
                let _ = writeln!(out, "- {reason}: {n}");
            }
        }
        out
    }

    pub fn to_json(&self) -> Value {
        let manifests: Vec<Value> = self
            .per_manifest()
            .into_iter()
            .map(|(m, c)| {
                let mut v = c.to_json();
                v["manifest"] = json!(m);
                v
            })
            .collect();
        let tests: Vec<Value> = self
            .records
            .iter()
            .map(|r| {
                json!({
                    "id": r.test.id,
                    "name": r.test.name,
                    "comment": r.test.comment,
                    "manifest": r.test.manifest,
                    "kind": match &r.test.kind {
                        TestKind::Query(_) => "query",
                        TestKind::ResultFormat(_) => "result-format",
                        TestKind::Update(_) => "update",
                        TestKind::Other(_) => "other",
                    },
                    "approval": r.test.approval,
                    "status": r.outcome.status.name(),
                    "message": r.outcome.message,
                    "known_failure": r.known_failure,
                    "ms": r.outcome.elapsed.as_millis() as u64,
                })
            })
            .collect();
        json!({
            "backend": self.backend,
            "suite": self.suite,
            "elapsed_ms": self.elapsed.as_millis() as u64,
            "totals": self.totals().to_json(),
            "pass_rate": self.totals().pass_rate(),
            "skip_reasons": self
                .skip_reasons()
                .into_iter()
                .map(|(reason, count)| json!({"reason": reason, "count": count}))
                .collect::<Vec<_>>(),
            "manifests": manifests,
            "tests": tests,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, manifest: &str, status: Status, known: bool) -> Record {
        Record {
            test: Arc::new(TestCase {
                id: id.into(),
                name: id.into(),
                comment: None,
                manifest: manifest.into(),
                approval: None,
                kind: TestKind::Other("x".into()),
                skip: None,
            }),
            outcome: Outcome {
                status,
                message: (status == Status::Skipped).then(|| "category: service".to_owned()),
                elapsed: Duration::from_millis(3),
            },
            known_failure: known,
        }
    }

    #[test]
    fn known_failures_file_format() {
        let known = parse_known_failures("# header\n\nhttp://a#t1   # flaky\n  http://a#t2\n#http://a#t3\n");
        assert_eq!(known, HashSet::from(["http://a#t1".to_owned(), "http://a#t2".to_owned()]));
    }

    #[test]
    fn aggregates_and_flags() {
        let report = Report {
            backend: "reference".into(),
            suite: "/suite".into(),
            elapsed: Duration::from_secs(1),
            records: vec![
                record("t1", "m1", Status::Pass, false),
                record("t2", "m1", Status::Fail, true),
                record("t3", "m2", Status::Error, false),
                record("t4", "m2", Status::Skipped, false),
                record("t5", "m2", Status::PassLenient, true),
            ],
        };
        let totals = report.totals();
        assert_eq!(totals, Counts { pass: 1, pass_lenient: 1, fail: 1, error: 1, skipped: 1 });
        assert_eq!(totals.run(), 4);
        assert!((totals.pass_rate() - 50.0).abs() < 1e-9);
        let per = report.per_manifest();
        assert_eq!(per.iter().map(|(m, c)| (m.as_str(), c.total())).collect::<Vec<_>>(), [("m1", 2), ("m2", 3)]);
        assert_eq!(report.unexpected_failures().map(|r| r.test.id.as_str()).collect::<Vec<_>>(), ["t3"]);
        assert_eq!(report.fixed_known_failures().map(|r| r.test.id.as_str()).collect::<Vec<_>>(), ["t5"]);
        assert_eq!(report.skip_reasons(), [("category: service".to_owned(), 1)]);

        let md = report.summary_markdown();
        assert!(md.contains("| m1 | 1 | 0 | 1 | 0 | 0 | 2 |"), "{md}");
        assert!(md.contains("| **total** | 1 | 1 | 1 | 1 | 1 | 5 |"), "{md}");
        let js = report.to_json();
        assert_eq!(js["totals"]["error"], 1);
        assert_eq!(js["tests"][1]["status"], "fail");
        assert_eq!(js["tests"][1]["known_failure"], true);
        assert_eq!(js["manifests"][1]["manifest"], "m2");
    }
}
