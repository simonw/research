//! pelicanscript — sandboxed Rhai scripting for Python.
//!
//! Embeds the Rhai engine (https://github.com/rhaiscript/rhai) as a CPython
//! extension module via PyO3. The point of the exercise is the *sandbox*: an
//! `Engine` is constructed with explicit CPU / RAM / recursion budgets and
//! hostile scripts raise typed Python exceptions instead of taking the
//! interpreter down with them.

use std::any::TypeId;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use pyo3::BoundObject;

use rhai::{Array, Dynamic, Engine as RhaiEngine, EvalAltResult, Map, Scope, INT};

// ---------------------------------------------------------------------------
// Exception hierarchy
// ---------------------------------------------------------------------------

pyo3::create_exception!(pelicanscript, RhaiError, pyo3::exceptions::PyException);
pyo3::create_exception!(pelicanscript, ScriptParseError, RhaiError);
pyo3::create_exception!(pelicanscript, ScriptRuntimeError, RhaiError);
pyo3::create_exception!(pelicanscript, ScriptTimeout, RhaiError);
pyo3::create_exception!(pelicanscript, TooManyOperations, RhaiError);
pyo3::create_exception!(pelicanscript, DataTooLarge, RhaiError);
pyo3::create_exception!(pelicanscript, StackOverflow, RhaiError);

/// Map a Rhai error onto the most specific Python exception we have.
fn to_py_err(err: Box<EvalAltResult>) -> PyErr {
    let msg = err.to_string();
    match *err {
        EvalAltResult::ErrorTooManyOperations(..) => TooManyOperations::new_err(msg),
        EvalAltResult::ErrorTerminated(..) => ScriptTimeout::new_err(msg),
        EvalAltResult::ErrorDataTooLarge(..) => DataTooLarge::new_err(msg),
        EvalAltResult::ErrorStackOverflow(..) | EvalAltResult::ErrorTooManyModules(..) => {
            StackOverflow::new_err(msg)
        }
        EvalAltResult::ErrorParsing(..) => ScriptParseError::new_err(msg),
        // A Python callback that raised is smuggled back out through
        // ErrorRuntime carrying the original exception's text.
        _ => ScriptRuntimeError::new_err(msg),
    }
}

// ---------------------------------------------------------------------------
// Value conversion: Python <-> Rhai Dynamic
// ---------------------------------------------------------------------------

fn py_to_dynamic(obj: &Bound<'_, PyAny>) -> PyResult<Dynamic> {
    if obj.is_none() {
        return Ok(Dynamic::UNIT);
    }
    // bool before int: Python bools are ints.
    if let Ok(b) = obj.cast::<PyBool>() {
        return Ok(Dynamic::from_bool(b.is_true()));
    }
    if obj.is_instance_of::<PyInt>() {
        let v: i64 = obj.extract()?;
        return Ok(Dynamic::from_int(v as INT));
    }
    if obj.is_instance_of::<PyFloat>() {
        let v: f64 = obj.extract()?;
        return Ok(Dynamic::from_float(v));
    }
    if obj.is_instance_of::<PyString>() {
        let v: String = obj.extract()?;
        return Ok(Dynamic::from(v));
    }
    if let Ok(list) = obj.cast::<PyList>() {
        let mut arr = Array::new();
        for item in list.iter() {
            arr.push(py_to_dynamic(&item)?);
        }
        return Ok(Dynamic::from_array(arr));
    }
    if let Ok(tuple) = obj.cast::<PyTuple>() {
        let mut arr = Array::new();
        for item in tuple.iter() {
            arr.push(py_to_dynamic(&item)?);
        }
        return Ok(Dynamic::from_array(arr));
    }
    if let Ok(dict) = obj.cast::<PyDict>() {
        let mut map = Map::new();
        for (k, v) in dict.iter() {
            let key: String = k.extract().map_err(|_| {
                PyTypeError::new_err("Rhai object map keys must be strings")
            })?;
            map.insert(key.into(), py_to_dynamic(&v)?);
        }
        return Ok(Dynamic::from_map(map));
    }
    Err(PyTypeError::new_err(format!(
        "cannot convert Python value of type '{}' to a Rhai value",
        obj.get_type().name()?
    )))
}

fn dynamic_to_py<'py>(py: Python<'py>, value: &Dynamic) -> PyResult<Bound<'py, PyAny>> {
    if value.is_unit() {
        return Ok(py.None().into_bound(py));
    }
    if let Some(b) = value.clone().try_cast::<bool>() {
        return Ok(PyBool::new(py, b).into_bound().into_any());
    }
    if let Some(i) = value.clone().try_cast::<INT>() {
        return Ok((i as i64).into_pyobject(py)?.into_any());
    }
    if let Some(f) = value.clone().try_cast::<f64>() {
        return Ok(f.into_pyobject(py)?.into_any());
    }
    if let Some(c) = value.clone().try_cast::<char>() {
        return Ok(c.to_string().into_pyobject(py)?.into_any());
    }
    if value.is_string() {
        let s = value.clone().into_string().map_err(PyValueError::new_err)?;
        return Ok(s.into_pyobject(py)?.into_any());
    }
    if let Some(arr) = value.clone().try_cast::<Array>() {
        let list = PyList::empty(py);
        for item in arr.iter() {
            list.append(dynamic_to_py(py, item)?)?;
        }
        return Ok(list.into_any());
    }
    if let Some(map) = value.clone().try_cast::<Map>() {
        let dict = PyDict::new(py);
        for (k, v) in map.iter() {
            dict.set_item(k.as_str(), dynamic_to_py(py, v)?)?;
        }
        return Ok(dict.into_any());
    }
    // Anything else (timestamps, fn pointers, custom types) degrades to its
    // string representation rather than failing the whole call.
    Ok(value.to_string().into_pyobject(py)?.into_any())
}

// ---------------------------------------------------------------------------
// The Engine
// ---------------------------------------------------------------------------

/// A sandboxed Rhai engine.
///
/// `unsendable` because Rhai's `Engine` uses `Rc` internally (the `sync`
/// feature is off); PyO3 will raise if it is touched from another thread.
#[pyclass(unsendable)]
struct Engine {
    engine: RhaiEngine,
    scope: Scope<'static>,
    output: Rc<RefCell<Vec<String>>>,
    /// Deadline for the *current* eval; set before each run so the timeout is
    /// per-call rather than per-engine.
    deadline: Rc<RefCell<Option<Instant>>>,
    timeout: Option<Duration>,
}

#[pymethods]
impl Engine {
    /// Construct an engine. Every limit is optional; `None` means "no limit"
    /// (Rhai's own default for that knob).
    #[new]
    #[pyo3(signature = (
        *,
        max_operations = None,
        timeout_ms = None,
        max_call_levels = None,
        max_array_size = None,
        max_map_size = None,
        max_string_size = None,
        max_expr_depth = None,
    ))]
    fn new(
        max_operations: Option<u64>,
        timeout_ms: Option<u64>,
        max_call_levels: Option<usize>,
        max_array_size: Option<usize>,
        max_map_size: Option<usize>,
        max_string_size: Option<usize>,
        max_expr_depth: Option<usize>,
    ) -> Self {
        let mut engine = RhaiEngine::new();

        if let Some(n) = max_operations {
            engine.set_max_operations(n);
        }
        if let Some(n) = max_call_levels {
            engine.set_max_call_levels(n);
        }
        if let Some(n) = max_array_size {
            engine.set_max_array_size(n);
        }
        if let Some(n) = max_map_size {
            engine.set_max_map_size(n);
        }
        if let Some(n) = max_string_size {
            engine.set_max_string_size(n);
        }
        if let Some(n) = max_expr_depth {
            engine.set_max_expr_depths(n, n);
        }

        // Capture script `print`/`debug` output for the Python side.
        let output = Rc::new(RefCell::new(Vec::new()));
        let sink = output.clone();
        engine.on_print(move |msg| sink.borrow_mut().push(msg.to_string()));
        let sink = output.clone();
        engine.on_debug(move |msg, src, pos| {
            sink.borrow_mut()
                .push(format!("[debug {}:{}] {msg}", src.unwrap_or("script"), pos))
        });

        // Wall-clock watchdog. `on_progress` is called on every operation, so
        // this bounds real time even when a script does no allocation.
        let deadline: Rc<RefCell<Option<Instant>>> = Rc::new(RefCell::new(None));
        let watch = deadline.clone();
        engine.on_progress(move |_ops| {
            match *watch.borrow() {
                Some(d) if Instant::now() >= d => {
                    Some(Dynamic::from("script exceeded its time limit"))
                }
                _ => None,
            }
        });

        Self {
            engine,
            scope: Scope::new(),
            output,
            deadline,
            timeout: timeout_ms.map(Duration::from_millis),
        }
    }

    /// Set a variable in the engine's persistent scope.
    fn set(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let v = py_to_dynamic(value)?;
        self.scope.set_or_push(name.to_string(), v);
        Ok(())
    }

    /// Read a variable back out of the scope (None if unset).
    fn get<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        match self.scope.get_value::<Dynamic>(name) {
            Some(v) => dynamic_to_py(py, &v),
            None => Ok(py.None().into_bound(py)),
        }
    }

    /// Expose a Python callable to scripts under `name`, taking `arity` args.
    ///
    /// The callback runs with the GIL held; if it raises, the exception text is
    /// carried back out through Rhai as a `ScriptRuntimeError`.
    #[pyo3(signature = (name, func, arity = 1))]
    fn register(&mut self, name: &str, func: Py<PyAny>, arity: usize) -> PyResult<()> {
        if arity > 8 {
            return Err(PyValueError::new_err("arity must be <= 8"));
        }
        // TypeId::of::<Dynamic>() is Rhai's wildcard parameter type.
        let arg_types = vec![TypeId::of::<Dynamic>(); arity];

        self.engine
            .register_raw_fn(name, arg_types, move |_ctx, args| {
                Python::attach(|py| {
                    let mut py_args: Vec<Bound<'_, PyAny>> = Vec::with_capacity(args.len());
                    for a in args.iter() {
                        py_args.push(dynamic_to_py(py, a).map_err(|e| {
                            Box::new(EvalAltResult::ErrorRuntime(
                                Dynamic::from(e.to_string()),
                                rhai::Position::NONE,
                            ))
                        })?);
                    }
                    let tuple = PyTuple::new(py, py_args).map_err(|e| {
                        Box::new(EvalAltResult::ErrorRuntime(
                            Dynamic::from(e.to_string()),
                            rhai::Position::NONE,
                        ))
                    })?;
                    let result = func.bind(py).call1(tuple).map_err(|e| {
                        Box::new(EvalAltResult::ErrorRuntime(
                            Dynamic::from(format!("Python callback raised: {e}")),
                            rhai::Position::NONE,
                        ))
                    })?;
                    py_to_dynamic(&result).map_err(|e| {
                        Box::new(EvalAltResult::ErrorRuntime(
                            Dynamic::from(e.to_string()),
                            rhai::Position::NONE,
                        ))
                    })
                })
            });
        Ok(())
    }

    /// Compile and run a script, returning its value as a Python object.
    fn eval<'py>(&mut self, py: Python<'py>, script: &str) -> PyResult<Bound<'py, PyAny>> {
        self.arm_deadline();
        let result = self
            .engine
            .eval_with_scope::<Dynamic>(&mut self.scope, script);
        self.disarm_deadline();
        match result {
            Ok(v) => dynamic_to_py(py, &v),
            Err(e) => Err(to_py_err(e)),
        }
    }

    /// Run a script for its side effects only.
    fn run(&mut self, script: &str) -> PyResult<()> {
        self.arm_deadline();
        let result = self.engine.run_with_scope(&mut self.scope, script);
        self.disarm_deadline();
        result.map_err(to_py_err)
    }

    /// Parse-check a script without running it. Raises `ScriptParseError`.
    fn check(&self, script: &str) -> PyResult<()> {
        self.engine
            .compile(script)
            .map(|_| ())
            .map_err(|e| ScriptParseError::new_err(e.to_string()))
    }

    /// Everything the script `print`ed since the last `clear_output()`.
    #[getter]
    fn output(&self) -> Vec<String> {
        self.output.borrow().clone()
    }

    fn clear_output(&self) {
        self.output.borrow_mut().clear();
    }

    /// Verify a Python value against this engine's size limits *before*
    /// handing it to a script — useful because Rhai's own map-size limit is
    /// not enforced on `m[key] = value` index assignment.
    fn check_size(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let v = py_to_dynamic(value)?;
        self.engine
            .ensure_data_size_within_limits(&v)
            .map_err(to_py_err)
    }
}

impl Engine {
    fn arm_deadline(&mut self) {
        *self.deadline.borrow_mut() = self.timeout.map(|t| Instant::now() + t);
    }
    fn disarm_deadline(&mut self) {
        *self.deadline.borrow_mut() = None;
    }
}

#[pymodule]
fn pelicanscript(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Engine>()?;
    m.add("RhaiError", py.get_type::<RhaiError>())?;
    m.add("ScriptParseError", py.get_type::<ScriptParseError>())?;
    m.add("ScriptRuntimeError", py.get_type::<ScriptRuntimeError>())?;
    m.add("ScriptTimeout", py.get_type::<ScriptTimeout>())?;
    m.add("TooManyOperations", py.get_type::<TooManyOperations>())?;
    m.add("DataTooLarge", py.get_type::<DataTooLarge>())?;
    m.add("StackOverflow", py.get_type::<StackOverflow>())?;
    m.add("__doc__", "Sandboxed Rhai scripting for Python, powered by rhai + PyO3")?;
    m.add("rhai_version", "1.25.1")?;
    Ok(())
}
