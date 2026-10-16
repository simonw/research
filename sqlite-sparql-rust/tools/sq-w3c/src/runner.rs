//! Executes tests against a backend. Every test runs on a worker thread that owns the
//! backend, inside `catch_unwind` (a `todo!()` or any panic becomes an `error` outcome, never
//! an aborted run) and under a timeout (the worker is interrupted, or abandoned and replaced).
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashSet;
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Once};
use std::thread;
use std::time::{Duration, Instant};

use oxrdf::{Dataset, NamedNode};
use spargebra::{GraphUpdateOperation, SparqlParser};

use crate::backend::{Backend, BackendError, BackendKind, Interrupter, QueryRequest, UpdateRequest};
use crate::compare::{compare_query, compare_result_text, compare_update, ColumnOrder, RowOrder, Verdict};
use crate::manifest::{DatasetFiles, GraphFile, QueryTest, TestCase, TestKind, UpdateTest};
use crate::results::{parse_expected, TextFormat};
use crate::suite::{parse_rdf_into, rdf_format, Suite};

const WORKER_NAME: &str = "sq-w3c-worker";
/// Deeply nested algebra (and the backtracking matcher) recurse; give workers room.
const WORKER_STACK_BYTES: usize = 256 << 20;
/// How long an interrupted worker gets to come back before it is abandoned.
const INTERRUPT_GRACE: Duration = Duration::from_secs(5);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Status {
    Pass,
    PassLenient,
    Fail,
    Error,
    Skipped,
}

impl Status {
    pub const fn name(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::PassLenient => "pass-lenient",
            Status::Fail => "fail",
            Status::Error => "error",
            Status::Skipped => "skipped",
        }
    }

    pub const fn is_failure(self) -> bool {
        matches!(self, Status::Fail | Status::Error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub status: Status,
    pub message: Option<String>,
    pub elapsed: Duration,
}

impl Outcome {
    fn new(status: Status, message: Option<String>) -> Self {
        Outcome { status, message, elapsed: Duration::ZERO }
    }

    pub fn skipped(reason: impl Into<String>) -> Self {
        Outcome::new(Status::Skipped, Some(reason.into()))
    }

    pub fn error(message: impl Into<String>) -> Self {
        Outcome::new(Status::Error, Some(message.into()))
    }
}

/// Why a test stopped before a verdict.
enum Abort {
    Skip(String),
    Error(String),
}

impl From<BackendError> for Abort {
    fn from(e: BackendError) -> Self {
        match e {
            BackendError::Unsupported(m) => Abort::Skip(format!("backend: {m}")),
            BackendError::Failed(m) => Abort::Error(m),
        }
    }
}

/// Harness-side failures (unreadable files, unparsable expectations) are errors too.
fn harness(e: String) -> Abort {
    Abort::Error(e)
}

/// Runs one test on `backend` in the current thread (no isolation).
pub fn run_test(suite: &Suite, backend: &mut dyn Backend, test: &TestCase) -> Outcome {
    if let Some(reason) = &test.skip {
        return Outcome::skipped(reason.clone());
    }
    let verdict = match &test.kind {
        TestKind::Query(q) => run_query(suite, backend, q, false),
        TestKind::ResultFormat(q) => run_query(suite, backend, q, true),
        TestKind::Update(u) => run_update(suite, backend, u),
        TestKind::Other(t) => Err(Abort::Skip(format!("test type: {t}"))),
    };
    match verdict {
        Ok(Verdict::Pass) => Outcome::new(Status::Pass, None),
        Ok(Verdict::PassLenient(m)) => Outcome::new(Status::PassLenient, Some(m)),
        Ok(Verdict::Fail(m)) => Outcome::new(Status::Fail, Some(m)),
        Err(Abort::Skip(m)) => Outcome::skipped(m),
        Err(Abort::Error(m)) => Outcome::error(m),
    }
}

fn load_file(suite: &Suite, backend: &mut dyn Backend, file: &str, graph: Option<&str>) -> Result<(), Abort> {
    let data = suite.read(file).map_err(harness)?;
    let format = rdf_format(file).ok_or_else(|| harness(format!("unknown RDF file type: {file}")))?;
    let graph = graph.map(NamedNode::new).transpose().map_err(|e| harness(format!("bad graph name: {e}")))?;
    backend.load(&data, format, file, graph.as_ref().map(NamedNode::as_ref)).map_err(|e| match e {
        BackendError::Failed(m) => Abort::Error(format!("loading {file}: {m}")),
        other => other.into(),
    })
}

fn load_dataset(suite: &Suite, backend: &mut dyn Backend, files: &[String], graphs: &[GraphFile]) -> Result<(), Abort> {
    for file in files {
        load_file(suite, backend, file, None)?;
    }
    for g in graphs {
        load_file(suite, backend, &g.file, Some(&g.name))?;
    }
    Ok(())
}

fn run_query(suite: &Suite, backend: &mut dyn Backend, test: &QueryTest, text_format: bool) -> Result<Verdict, Abort> {
    let result = test.result.as_deref().ok_or_else(|| harness("test has no mf:result".into()))?;
    let text = suite.read_text(&test.query).map_err(harness)?;
    let parsed = SparqlParser::new()
        .with_base_iri(test.query.as_str())
        .map_err(|e| harness(e.to_string()))?
        .parse_query(&text)
        .map_err(|e| harness(format!("the query does not parse: {e}")))?;

    backend.reset()?;
    load_dataset(suite, backend, &test.data, &test.graph_data)?;
    // FROM / FROM NAMED files of the suite that the manifest does not load are preloaded as
    // named graphs (dataset-09b, dup-bnode tests): the query's dataset clause selects them.
    if let Some(dataset) = parsed.dataset() {
        let mut loaded: HashSet<&str> = test.graph_data.iter().map(|g| g.name.as_str()).collect();
        for graph in dataset.default.iter().chain(dataset.named.iter().flatten()) {
            if suite.relative(graph.as_str()).is_some() && loaded.insert(graph.as_str()) {
                load_file(suite, backend, graph.as_str(), Some(graph.as_str()))?;
            }
        }
    }

    let request = QueryRequest { text: &text, base: &test.query, parsed: &parsed };
    if text_format {
        let expected = suite.read_text(result).map_err(harness)?;
        let format = TextFormat::of_file(result)
            .ok_or_else(|| harness(format!("result-format test with unknown result type: {result}")))?;
        let actual = backend.query_text(&request, format)?;
        return Ok(match compare_result_text(&expected, &actual, format, ColumnOrder::of_query(&text)) {
            Ok(()) => Verdict::Pass,
            Err(m) => Verdict::Fail(m),
        });
    }
    let expected = parse_expected(&suite.read(result).map_err(harness)?, result).map_err(harness)?;
    let actual = backend.query(&request)?;
    Ok(compare_query(&expected, &actual, &RowOrder::of(&parsed), test.lax_cardinality))
}

fn expected_dataset(suite: &Suite, files: &DatasetFiles) -> Result<Dataset, Abort> {
    let mut dataset = Dataset::new();
    let mut add = |file: &str, graph: Option<&str>| -> Result<(), String> {
        let data = suite.read(file)?;
        let format = rdf_format(file).ok_or_else(|| format!("unknown RDF file type: {file}"))?;
        let graph = graph.map(NamedNode::new).transpose().map_err(|e| format!("bad graph name: {e}"))?;
        parse_rdf_into(&mut dataset, &data, format, file, graph.as_ref().map(NamedNode::as_ref))
    };
    for file in &files.data {
        add(file, None).map_err(harness)?;
    }
    for g in &files.graph_data {
        add(&g.file, Some(&g.name)).map_err(harness)?;
    }
    Ok(dataset)
}

fn run_update(suite: &Suite, backend: &mut dyn Backend, test: &UpdateTest) -> Result<Verdict, Abort> {
    let text = suite.read_text(&test.request).map_err(harness)?;
    let parsed = SparqlParser::new()
        .with_base_iri(test.request.as_str())
        .map_err(|e| harness(e.to_string()))?
        .parse_update(&text)
        .map_err(|e| harness(format!("the update does not parse: {e}")))?;
    let remote_load = parsed.operations.iter().any(|op| {
        matches!(op, GraphUpdateOperation::Load { source, .. }
            if source.as_str().starts_with("http://") || source.as_str().starts_with("https://"))
    });
    if remote_load {
        return Err(Abort::Skip("category: http(s) LOAD".into()));
    }
    let expected = expected_dataset(suite, &test.after)?;

    backend.reset()?;
    load_dataset(suite, backend, &test.before.data, &test.before.graph_data)?;
    backend.update(&UpdateRequest { text: &text, base: &test.request, parsed: &parsed })?;
    let actual = backend.dump()?;
    Ok(compare_update(&expected, &actual))
}

// ------------------------------------------------------------------ isolation

thread_local! {
    /// Message and location of the last panic on this worker thread.
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Installs (once) a panic hook that records worker-thread panics instead of printing them;
/// panics on other threads keep the default behaviour.
fn install_panic_hook(echo: bool) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if thread::current().name() != Some(WORKER_NAME) {
                return default(info);
            }
            let location = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
            let message = format!("panic: {}{location}", payload_text(info.payload()));
            if echo {
                eprintln!("{message}");
            }
            LAST_PANIC.with(|p| *p.borrow_mut() = Some(message));
        }));
    });
}

fn payload_text(payload: &dyn Any) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

/// Runs `f`, turning a panic into `Err(message)`.
fn isolate<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    LAST_PANIC.with(|p| p.borrow_mut().take());
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|payload| {
        LAST_PANIC.with(|p| p.borrow_mut().take()).unwrap_or_else(|| format!("panic: {}", payload_text(&*payload)))
    })
}

struct Worker {
    jobs: Sender<Arc<TestCase>>,
    results: Receiver<Outcome>,
    interrupter: Option<Interrupter>,
}

impl Worker {
    fn spawn(factory: Arc<BackendFactory>, suite: Arc<Suite>) -> Result<Worker, String> {
        let (job_tx, job_rx) = mpsc::channel::<Arc<TestCase>>();
        let (result_tx, result_rx) = mpsc::channel::<Outcome>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<Option<Interrupter>, String>>();
        thread::Builder::new()
            .name(WORKER_NAME.into())
            .stack_size(WORKER_STACK_BYTES)
            .spawn(move || {
                let create = || isolate(|| factory()).and_then(|r| r);
                let mut backend = match create() {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                if ready_tx.send(Ok(backend.interrupter())).is_err() {
                    return;
                }
                for test in job_rx {
                    let start = Instant::now();
                    let mut outcome = match isolate(|| run_test(&suite, backend.as_mut(), &test)) {
                        Ok(outcome) => outcome,
                        Err(panic) => {
                            // The backend may be half-way through an operation: replace it.
                            if let Ok(fresh) = create() {
                                backend = fresh;
                            }
                            Outcome::error(panic)
                        }
                    };
                    outcome.elapsed = start.elapsed();
                    if result_tx.send(outcome).is_err() {
                        return; // abandoned after a timeout
                    }
                }
            })
            .map_err(|e| format!("cannot spawn a worker thread: {e}"))?;
        let interrupter = ready_rx.recv().map_err(|_| "worker thread died during start-up".to_owned())??;
        Ok(Worker { jobs: job_tx, results: result_rx, interrupter })
    }
}

/// Creates a backend; called on the worker thread (backends need not be `Send`).
pub type BackendFactory = dyn Fn() -> Result<Box<dyn Backend>, String> + Send + Sync;

/// Runs tests one at a time on a worker thread with a per-test timeout.
pub struct Runner {
    factory: Arc<BackendFactory>,
    suite: Arc<Suite>,
    timeout: Duration,
    worker: Option<Worker>,
}

impl Runner {
    pub fn new(kind: BackendKind, suite: Suite, timeout: Duration, echo_panics: bool) -> Result<Self, String> {
        Self::with_factory(Arc::new(move || kind.create()), suite, timeout, echo_panics)
    }

    pub fn with_factory(
        factory: Arc<BackendFactory>,
        suite: Suite,
        timeout: Duration,
        echo_panics: bool,
    ) -> Result<Self, String> {
        install_panic_hook(echo_panics);
        let suite = Arc::new(suite);
        let worker = Worker::spawn(Arc::clone(&factory), Arc::clone(&suite))?;
        Ok(Runner { factory, suite, timeout, worker: Some(worker) })
    }

    pub fn run(&mut self, test: &Arc<TestCase>) -> Outcome {
        if let Some(reason) = &test.skip {
            return Outcome::skipped(reason.clone());
        }
        let worker = match self.worker.take() {
            Some(w) => w,
            None => match Worker::spawn(Arc::clone(&self.factory), Arc::clone(&self.suite)) {
                Ok(w) => w,
                Err(e) => return Outcome::error(e),
            },
        };
        if worker.jobs.send(Arc::clone(test)).is_err() {
            return Outcome::error("worker thread is gone");
        }
        match worker.results.recv_timeout(self.timeout) {
            Ok(outcome) => {
                self.worker = Some(worker);
                outcome
            }
            Err(RecvTimeoutError::Timeout) => {
                let message = format!("timeout after {} s", self.timeout.as_secs_f64());
                if let Some(interrupt) = &worker.interrupter {
                    interrupt();
                    if worker.results.recv_timeout(INTERRUPT_GRACE).is_ok() {
                        // The backend honoured the interrupt: keep the worker.
                        self.worker = Some(worker);
                    }
                }
                // Otherwise the worker is abandoned; a new one starts with the next test.
                let mut outcome = Outcome::error(message);
                outcome.elapsed = self.timeout;
                outcome
            }
            Err(RecvTimeoutError::Disconnected) => Outcome::error("worker thread died"),
        }
    }
}

#[cfg(all(test, feature = "reference"))]
mod tests {
    use super::*;
    use crate::manifest::load_tests;

    /// A throwaway suite on disk: one manifest with the given entries and files.
    struct Fixture {
        dir: std::path::PathBuf,
        suite: Suite,
    }

    impl Fixture {
        fn new(tag: &str, files: &[(&str, &str)]) -> Fixture {
            let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let dir = std::env::temp_dir().join(format!("sq-w3c-runner-{tag}-{}-{n}", std::process::id()));
            for (rel, text) in files {
                let path = dir.join(rel);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, text).unwrap();
            }
            let suite = Suite::new(&dir).unwrap();
            Fixture { dir, suite }
        }

        fn tests(&self) -> Vec<TestCase> {
            load_tests(&self.suite, &[self.suite.url("t/manifest.ttl")]).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    const MANIFEST: &str = r#"
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix mf: <http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#> .
@prefix qt: <http://www.w3.org/2001/sw/DataAccess/tests/test-query#> .
@prefix ut: <http://www.w3.org/2009/sparql/tests/test-update#> .
@prefix : <http://example.org/t#> .
<> a mf:Manifest ; mf:entries (:select :wrong :ordered :from :construct :update :csv :http-load) .
:select a mf:QueryEvaluationTest ; mf:name "select" ;
    mf:action [ qt:query <select.rq> ; qt:data <data.ttl> ; qt:graphData <g.ttl> ] ; mf:result <select.srj> .
:wrong a mf:QueryEvaluationTest ; mf:name "wrong" ;
    mf:action [ qt:query <select.rq> ; qt:data <data.ttl> ] ; mf:result <wrong.srj> .
:ordered a mf:QueryEvaluationTest ; mf:name "ordered" ;
    mf:action [ qt:query <ordered.rq> ; qt:data <data.ttl> ] ; mf:result <ordered.srj> .
:from a mf:QueryEvaluationTest ; mf:name "from" ;
    mf:action [ qt:query <from.rq> ] ; mf:result <from.srj> .
:construct a mf:QueryEvaluationTest ; mf:name "construct" ;
    mf:action [ qt:query <construct.rq> ; qt:data <data.ttl> ] ; mf:result <construct.ttl> .
:update a mf:UpdateEvaluationTest ; mf:name "update" ;
    mf:action [ ut:request <update.ru> ; ut:data <plain.ttl> ] ;
    mf:result [ ut:data <plain.ttl> ; ut:graphData [ ut:graph <plain.ttl> ; rdfs:label "http://example.org/copy" ] ] .
:csv a mf:CSVResultFormatTest ; mf:name "csv" ;
    mf:action [ qt:query <ordered.rq> ; qt:data <data.ttl> ] ; mf:result <ordered.csv> .
:http-load a mf:UpdateEvaluationTest ; mf:name "http load" ;
    mf:action [ ut:request <load.ru> ] ; mf:result [] .
"#;

    fn fixture() -> Fixture {
        Fixture::new(
            "basic",
            &[
                ("t/manifest.ttl", MANIFEST),
                ("t/data.ttl", "@prefix : <http://example.org/> . :a :p 1 . :b :p 2.0 . _:x :p :a ."),
                ("t/g.ttl", "<http://example.org/c> <http://example.org/p> 3 ."),
                ("t/plain.ttl", "@prefix : <http://example.org/> . :a :p 1 ; :q \"x\"@en ."),
                ("t/select.rq", "SELECT ?s ?o WHERE { ?s <http://example.org/p> ?o }"),
                (
                    "t/select.srj",
                    r#"{"head":{"vars":["s","o"]},"results":{"bindings":[
                    {"s":{"type":"uri","value":"http://example.org/a"},"o":{"type":"literal","value":"1","datatype":"http://www.w3.org/2001/XMLSchema#integer"}},
                    {"s":{"type":"uri","value":"http://example.org/b"},"o":{"type":"literal","value":"2.00","datatype":"http://www.w3.org/2001/XMLSchema#decimal"}},
                    {"s":{"type":"bnode","value":"q"},"o":{"type":"uri","value":"http://example.org/a"}}]}}"#,
                ),
                ("t/wrong.srj", r#"{"head":{"vars":["s","o"]},"results":{"bindings":[]}}"#),
                ("t/ordered.rq", "SELECT ?s WHERE { ?s <http://example.org/p> ?o FILTER(isIRI(?s)) } ORDER BY DESC(?s)"),
                (
                    "t/ordered.srj",
                    r#"{"head":{"vars":["s"]},"results":{"bindings":[{"s":{"type":"uri","value":"http://example.org/b"}},{"s":{"type":"uri","value":"http://example.org/a"}}]}}"#,
                ),
                ("t/ordered.csv", "s\nhttp://example.org/b\nhttp://example.org/a\n"),
                ("t/from.rq", "SELECT ?o FROM <g.ttl> WHERE { ?s ?p ?o }"),
                (
                    "t/from.srj",
                    r#"{"head":{"vars":["o"]},"results":{"bindings":[{"o":{"type":"literal","value":"3","datatype":"http://www.w3.org/2001/XMLSchema#integer"}}]}}"#,
                ),
                ("t/construct.rq", "CONSTRUCT { ?o ?p ?s } WHERE { ?s ?p ?o FILTER(isIRI(?o)) }"),
                ("t/construct.ttl", "<http://example.org/a> <http://example.org/p> [] ."),
                ("t/update.ru", "INSERT { GRAPH <http://example.org/copy> { ?s ?p ?o } } WHERE { ?s ?p ?o }"),
                ("t/load.ru", "LOAD <http://example.org/remote.ttl>"),
            ],
        )
    }

    #[test]
    fn reference_backend_runs_every_kind() {
        let fx = fixture();
        let mut backend = BackendKind::Reference.create().unwrap();
        let outcomes: Vec<(String, Outcome)> =
            fx.tests().iter().map(|t| (t.name.clone(), run_test(&fx.suite, backend.as_mut(), t))).collect();
        let status = |name: &str| outcomes.iter().find(|(n, _)| n == name).map(|(_, o)| o.clone()).unwrap();
        assert_eq!(status("select").status, Status::PassLenient, "{:?}", status("select"));
        assert_eq!(status("wrong").status, Status::Fail);
        assert!(status("wrong").message.unwrap().contains("expected 0 rows, got 3"));
        assert_eq!(status("ordered").status, Status::Pass, "{:?}", status("ordered"));
        assert_eq!(status("from").status, Status::Pass, "{:?}", status("from"));
        assert_eq!(status("construct").status, Status::Pass, "{:?}", status("construct"));
        assert_eq!(status("update").status, Status::Pass, "{:?}", status("update"));
        assert_eq!(status("csv").status, Status::Pass, "{:?}", status("csv"));
        assert_eq!(status("http load").status, Status::Skipped);
    }

    struct Panicking;
    impl Backend for Panicking {
        fn reset(&mut self) -> crate::backend::BackendResult<()> {
            todo!("reset is not written yet")
        }
        fn load(
            &mut self,
            _: &[u8],
            _: oxrdfio::RdfFormat,
            _: &str,
            _: Option<oxrdf::NamedNodeRef<'_>>,
        ) -> crate::backend::BackendResult<()> {
            unreachable!()
        }
        fn query(&mut self, _: &QueryRequest<'_>) -> crate::backend::BackendResult<crate::results::QueryOutcome> {
            unreachable!()
        }
        fn update(&mut self, _: &UpdateRequest<'_>) -> crate::backend::BackendResult<()> {
            unreachable!()
        }
        fn dump(&mut self) -> crate::backend::BackendResult<Dataset> {
            unreachable!()
        }
    }

    #[test]
    fn panics_become_errors() {
        install_panic_hook(false);
        let fx = fixture();
        let test = fx.tests().into_iter().next().unwrap();
        let suite = fx.suite.clone();
        let outcome = thread::Builder::new()
            .name(WORKER_NAME.into())
            .spawn(move || isolate(|| run_test(&suite, &mut Panicking, &test)))
            .unwrap()
            .join()
            .unwrap();
        let message = outcome.unwrap_err();
        assert!(message.contains("not yet implemented: reset is not written yet"), "{message}");
        assert!(message.contains("runner.rs"), "{message}");
    }

    #[test]
    fn runner_runs_on_a_worker() {
        let fx = fixture();
        let tests: Vec<Arc<TestCase>> = fx.tests().into_iter().map(Arc::new).collect();
        let mut runner = Runner::new(BackendKind::Reference, fx.suite.clone(), Duration::from_secs(60), false).unwrap();
        assert_eq!(runner.run(&tests[0]).status, Status::PassLenient);
        assert_eq!(runner.run(&tests[1]).status, Status::Fail);
    }

    /// Delegates to the reference backend, but its first `reset` hangs for up to 3 s
    /// (returning early when interrupted, if interruptible).
    struct Slow {
        inner: Box<dyn Backend>,
        hang: bool,
        stop: Option<Arc<std::sync::atomic::AtomicBool>>,
    }

    impl Backend for Slow {
        fn reset(&mut self) -> crate::backend::BackendResult<()> {
            if std::mem::replace(&mut self.hang, false) {
                for _ in 0..300 {
                    if self.stop.as_ref().is_some_and(|s| s.load(std::sync::atomic::Ordering::SeqCst)) {
                        return Err(BackendError::Failed("interrupted".into()));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
            self.inner.reset()
        }
        fn load(
            &mut self,
            data: &[u8],
            format: oxrdfio::RdfFormat,
            base: &str,
            graph: Option<oxrdf::NamedNodeRef<'_>>,
        ) -> crate::backend::BackendResult<()> {
            self.inner.load(data, format, base, graph)
        }
        fn query(&mut self, r: &QueryRequest<'_>) -> crate::backend::BackendResult<crate::results::QueryOutcome> {
            self.inner.query(r)
        }
        fn update(&mut self, r: &UpdateRequest<'_>) -> crate::backend::BackendResult<()> {
            self.inner.update(r)
        }
        fn dump(&mut self) -> crate::backend::BackendResult<Dataset> {
            self.inner.dump()
        }
        fn interrupter(&self) -> Option<Interrupter> {
            let stop = Arc::clone(self.stop.as_ref()?);
            Some(Box::new(move || stop.store(true, std::sync::atomic::Ordering::SeqCst)))
        }
    }

    /// A factory whose FIRST backend hangs; returns it with its creation counter.
    fn slow_factory(interruptible: bool) -> (Arc<BackendFactory>, Arc<std::sync::atomic::AtomicUsize>) {
        let created = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&created);
        let factory: Arc<BackendFactory> = Arc::new(move || {
            let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Box::new(Slow {
                inner: BackendKind::Reference.create()?,
                hang: n == 0,
                stop: interruptible.then(|| Arc::new(std::sync::atomic::AtomicBool::new(false))),
            }) as Box<dyn Backend>)
        });
        (factory, created)
    }

    #[test]
    fn timeout_interrupts_and_keeps_the_worker() {
        let fx = fixture();
        let tests: Vec<Arc<TestCase>> = fx.tests().into_iter().map(Arc::new).collect();
        let (factory, created) = slow_factory(true);
        let mut runner = Runner::with_factory(factory, fx.suite.clone(), Duration::from_millis(100), false).unwrap();
        let outcome = runner.run(&tests[0]);
        assert_eq!(outcome.status, Status::Error);
        assert!(outcome.message.unwrap().starts_with("timeout"));
        assert_eq!(runner.run(&tests[1]).status, Status::Fail);
        assert_eq!(created.load(std::sync::atomic::Ordering::SeqCst), 1, "the interrupted worker was reused");
    }

    #[test]
    fn timeout_abandons_an_unresponsive_worker() {
        let fx = fixture();
        let tests: Vec<Arc<TestCase>> = fx.tests().into_iter().map(Arc::new).collect();
        let (factory, created) = slow_factory(false);
        let mut runner = Runner::with_factory(factory, fx.suite.clone(), Duration::from_millis(100), false).unwrap();
        assert!(runner.run(&tests[0]).message.unwrap().starts_with("timeout"));
        assert_eq!(runner.run(&tests[1]).status, Status::Fail);
        assert_eq!(created.load(std::sync::atomic::Ordering::SeqCst), 2, "a fresh worker replaced the stuck one");
    }
}
