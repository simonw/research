//! Python bindings for the wasmi 2.0 WebAssembly interpreter.
//!
//! Design goals:
//! - Deterministic CPU limits via wasmi fuel metering.
//! - Wall-clock limits by running the guest in fuel *slices* and checking the
//!   clock between slices (wasmi 2.0 makes "out of fuel" a resumable state).
//! - Memory limits via wasmi's `ResourceLimiter` (`StoreLimits`).
//! - Host functions implemented as plain Python callables.
//! - Re-entrancy: a Python host function may call back into the guest
//!   (needed for emscripten-style setjmp/longjmp emulation).

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyKeyError, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};
use std::cell::RefCell;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use wasmi::errors::{ErrorKind, HostError};
use wasmi::{
    AsContextMut, Caller, CompilationMode, Config, Engine as WEngine, Error as WError,
    Extern, ExternType, Func, FuncType, Instance as WInstance, Linker, Module as WModule,
    Ref, ResumableCall, ResumableCallOutOfFuel, Store as WStore, StoreContextMut, StoreLimits,
    StoreLimitsBuilder, TrapCode, Val, ValType, F32, F64, V128,
};

create_exception!(_core, WasmError, PyException, "Base class for all wasmi_sandbox errors.");
create_exception!(_core, Trap, WasmError, "The guest hit a WebAssembly trap (args: message, trap code name).");
create_exception!(_core, OutOfFuel, WasmError, "The guest exhausted its fuel budget.");
create_exception!(_core, Timeout, WasmError, "The guest exceeded its wall-clock deadline.");
create_exception!(_core, Exit, WasmError, "The guest exited with an i32 status (args: code).");
create_exception!(_core, LinkError, WasmError, "Instantiation / linking failed.");
create_exception!(_core, LongjmpUnwind, WasmError, "Internal: emscripten-style longjmp unwinding the guest stack.");

/// Marker host error: the real Python exception lives in `HostData::pending_err`.
#[derive(Debug)]
struct PyHostError;

impl fmt::Display for PyHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "python host function raised an exception")
    }
}

impl HostError for PyHostError {}

/// Data owned by each wasmi `Store`.
struct HostData {
    limits: StoreLimits,
    funcs: Vec<Py<PyAny>>,
    pending_err: Option<PyErr>,
    /// Remaining fuel budget (None = unlimited).
    budget: Option<u64>,
    /// Total fuel consumed across all calls.
    consumed: u64,
    /// Wall-clock deadline of the current top-level call.
    deadline: Option<Instant>,
    /// Fuel granted per slice when a deadline is active.
    slice: u64,
    fuel_enabled: bool,
    invokes: u64,
    unwinds: u64,
    stack_guard_hits: u64,
}

static STORE_IDS: AtomicUsize = AtomicUsize::new(1);

/// Minimum native stack that must remain before we re-enter the interpreter
/// from a host function. Guest recursion re-enters wasmi once per
/// emscripten-style invoke, and the guest cannot see the host's stack.
const MIN_NATIVE_STACK: usize = 768 * 1024;

fn native_stack_low() -> bool {
    stacker::remaining_stack().map_or(false, |r| r < MIN_NATIVE_STACK)
}

thread_local! {
    /// Stack of (store id, pointer to the active `Caller`) for re-entrant access
    /// to the store while a Python host function is running.
    static CALLERS: RefCell<Vec<(usize, *mut Caller<'static, HostData>)>> = const { RefCell::new(Vec::new()) };
}

// ---------------------------------------------------------------------------
// value conversions
// ---------------------------------------------------------------------------

fn valtype_name(ty: &ValType) -> &'static str {
    match ty {
        ValType::I32 => "i32",
        ValType::I64 => "i64",
        ValType::F32 => "f32",
        ValType::F64 => "f64",
        ValType::V128 => "v128",
        ValType::FuncRef => "funcref",
        ValType::ExternRef => "externref",
    }
}

fn parse_valtype(name: &str) -> PyResult<ValType> {
    Ok(match name {
        "i32" => ValType::I32,
        "i64" => ValType::I64,
        "f32" => ValType::F32,
        "f64" => ValType::F64,
        "funcref" => ValType::FuncRef,
        "externref" => ValType::ExternRef,
        other => return Err(PyValueError::new_err(format!("unknown value type {other:?}"))),
    })
}

fn py_to_val(obj: &Bound<'_, PyAny>, ty: &ValType) -> PyResult<Val> {
    Ok(match ty {
        ValType::I32 => {
            let v: i64 = obj.extract()?;
            Val::I32(v as i32)
        }
        ValType::I64 => {
            if let Ok(v) = obj.extract::<i64>() {
                Val::I64(v)
            } else {
                let v: u64 = obj.extract()?;
                Val::I64(v as i64)
            }
        }
        ValType::F32 => Val::F32(F32::from_float(obj.extract::<f64>()? as f32)),
        ValType::F64 => Val::F64(F64::from_float(obj.extract::<f64>()?)),
        ValType::FuncRef => {
            if obj.is_none() {
                Val::FuncRef(wasmi::Nullable::Null)
            } else {
                return Err(PyTypeError::new_err("only None (null) funcref values are supported"));
            }
        }
        ValType::ExternRef => {
            if obj.is_none() {
                Val::ExternRef(wasmi::Nullable::Null)
            } else {
                return Err(PyTypeError::new_err("only None (null) externref values are supported"));
            }
        }
        ValType::V128 => return Err(PyTypeError::new_err("v128 values are not supported")),
    })
}

fn val_to_py(py: Python<'_>, val: &Val) -> PyResult<Py<PyAny>> {
    Ok(match val {
        Val::I32(v) => v.into_pyobject(py)?.into_any().unbind(),
        Val::I64(v) => v.into_pyobject(py)?.into_any().unbind(),
        Val::F32(v) => (v.to_float() as f64).into_pyobject(py)?.into_any().unbind(),
        Val::F64(v) => v.to_float().into_pyobject(py)?.into_any().unbind(),
        Val::V128(_) => return Err(PyTypeError::new_err("v128 values are not supported")),
        Val::FuncRef(_) | Val::ExternRef(_) => py.None(),
    })
}

fn default_val(ty: &ValType) -> Val {
    match ty {
        ValType::I32 => Val::I32(0),
        ValType::I64 => Val::I64(0),
        ValType::F32 => Val::F32(F32::from_float(0.0)),
        ValType::F64 => Val::F64(F64::from_float(0.0)),
        ValType::V128 => Val::V128(V128::from(0u128)),
        ValType::FuncRef => Val::FuncRef(wasmi::Nullable::Null),
        ValType::ExternRef => Val::ExternRef(wasmi::Nullable::Null),
    }
}

fn results_to_py(py: Python<'_>, results: &[Val]) -> PyResult<Py<PyAny>> {
    match results.len() {
        0 => Ok(py.None()),
        1 => val_to_py(py, &results[0]),
        _ => {
            let items = results.iter().map(|v| val_to_py(py, v)).collect::<PyResult<Vec<_>>>()?;
            Ok(PyTuple::new(py, items)?.into_any().unbind())
        }
    }
}

fn trap_code_name(code: TrapCode) -> String {
    format!("{code:?}")
}

/// Convert a wasmi error into a Python exception, recovering a pending Python
/// exception raised by a host function if there is one.
fn convert_error(err: WError, data: &mut HostData) -> PyErr {
    if err.downcast_ref::<PyHostError>().is_some() {
        if let Some(e) = data.pending_err.take() {
            return e;
        }
        return PyRuntimeError::new_err("host function failed without a pending exception");
    }
    // A pending error can also surface wrapped in other error kinds (e.g. when
    // a nested call failed); prefer it if present.
    if let Some(e) = data.pending_err.take() {
        return e;
    }
    match err.kind() {
        ErrorKind::I32ExitStatus(code) => Exit::new_err((*code,)),
        ErrorKind::TrapCode(code) => Trap::new_err((err.to_string(), trap_code_name(*code))),
        ErrorKind::Fuel(_) | ErrorKind::ResumableOutOfFuel(_) => OutOfFuel::new_err(err.to_string()),
        ErrorKind::Linker(_) | ErrorKind::Instantiation(_) => LinkError::new_err(err.to_string()),
        _ => WasmError::new_err(err.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

#[pyclass(unsendable, module = "wasmi_sandbox._core")]
pub struct Engine {
    inner: WEngine,
    consume_fuel: bool,
}

#[pymethods]
impl Engine {
    /// Create a wasmi engine.
    ///
    /// consume_fuel: enable fuel metering (required for fuel budgets and timeouts).
    /// compilation_mode: "eager" (default; see note in run_call), "lazy" or "lazy_translation".
    ///
    /// max_recursion_depth: wasm call depth before a StackOverflow trap (wasmi default 1000).
    /// max_stack_height: wasmi value-stack size in bytes (wasmi default 1 MiB).
    #[new]
    #[pyo3(signature = (consume_fuel = true, compilation_mode = "eager", memory64 = true, max_recursion_depth = 1000, max_stack_height = 1 << 20))]
    fn new(consume_fuel: bool, compilation_mode: &str, memory64: bool, max_recursion_depth: usize, max_stack_height: usize) -> PyResult<Self> {
        let mut config = Config::default();
        config.consume_fuel(consume_fuel);
        config.wasm_memory64(memory64);
        config.set_max_recursion_depth(max_recursion_depth);
        config.set_max_stack_height(max_stack_height.max(1 << 20));
        let mode = match compilation_mode {
            "eager" => CompilationMode::Eager,
            "lazy" => CompilationMode::Lazy,
            "lazy_translation" => CompilationMode::LazyTranslation,
            other => {
                return Err(PyValueError::new_err(format!("unknown compilation mode {other:?}")))
            }
        };
        config.compilation_mode(mode);
        Ok(Engine { inner: WEngine::new(&config), consume_fuel })
    }

    #[getter]
    fn consume_fuel(&self) -> bool {
        self.consume_fuel
    }
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

#[pyclass(unsendable, module = "wasmi_sandbox._core")]
pub struct Module {
    inner: WModule,
}

fn extern_type_info(py: Python<'_>, ty: &ExternType) -> PyResult<Py<PyDict>> {
    let d = PyDict::new(py);
    match ty {
        ExternType::Func(ft) => {
            d.set_item("kind", "func")?;
            d.set_item("params", ft.params().iter().map(valtype_name).collect::<Vec<_>>())?;
            d.set_item("results", ft.results().iter().map(valtype_name).collect::<Vec<_>>())?;
        }
        ExternType::Memory(mt) => {
            d.set_item("kind", "memory")?;
            d.set_item("minimum", mt.minimum())?;
            d.set_item("maximum", mt.maximum())?;
            d.set_item("is_64", mt.is_64())?;
        }
        ExternType::Table(tt) => {
            d.set_item("kind", "table")?;
            d.set_item("minimum", tt.minimum())?;
            d.set_item("maximum", tt.maximum())?;
        }
        ExternType::Global(gt) => {
            d.set_item("kind", "global")?;
            d.set_item("type", valtype_name(&gt.content()))?;
            d.set_item("mutable", gt.mutability().is_mut())?;
        }
    }
    Ok(d.unbind())
}

#[pymethods]
impl Module {
    /// Compile a module from `.wasm` bytes (or `.wat` text as bytes).
    #[new]
    fn new(engine: &Engine, wasm: &[u8]) -> PyResult<Self> {
        let inner = WModule::new(&engine.inner, wasm).map_err(|e| WasmError::new_err(e.to_string()))?;
        Ok(Module { inner })
    }

    /// Validate module bytes without compiling. Raises on failure.
    #[staticmethod]
    fn validate(engine: &Engine, wasm: &[u8]) -> PyResult<()> {
        WModule::validate(&engine.inner, wasm).map_err(|e| WasmError::new_err(e.to_string()))
    }

    /// List of dicts describing every import: module, name, kind, and type info.
    fn imports(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for imp in self.inner.imports() {
            let d = extern_type_info(py, imp.ty())?;
            let d = d.bind(py);
            d.set_item("module", imp.module())?;
            d.set_item("name", imp.name())?;
            list.append(d)?;
        }
        Ok(list.unbind())
    }

    /// List of dicts describing every export.
    fn exports(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for exp in self.inner.exports() {
            let d = extern_type_info(py, exp.ty())?;
            let d = d.bind(py);
            d.set_item("name", exp.name())?;
            list.append(d)?;
        }
        Ok(list.unbind())
    }
}

// ---------------------------------------------------------------------------
// Store (engine store + linker + single instance)
// ---------------------------------------------------------------------------

#[pyclass(unsendable, module = "wasmi_sandbox._core")]
pub struct Store {
    id: usize,
    store: RefCell<WStore<HostData>>,
    linker: RefCell<Linker<HostData>>,
    instance: RefCell<Option<WInstance>>,
    #[pyo3(get)]
    fuel_enabled: bool,
}

/// Run `func` with `args`, honouring the store's fuel budget and deadline.
///
/// This is used both for top-level calls and for re-entrant calls made from
/// Python host functions. When a deadline is set the call is executed in fuel
/// slices; wasmi 2.0 lets us resume an out-of-fuel call, so we can check the
/// clock in between slices without any signal/thread machinery.
fn run_call(
    mut ctx: StoreContextMut<'_, HostData>,
    func: Func,
    args: &[Val],
) -> PyResult<Vec<Val>> {
    let ty = func.ty(&ctx);
    let mut outputs: Vec<Val> = ty.results().iter().map(default_val).collect();

    if !ctx.data().fuel_enabled {
        return match func.call(&mut ctx, args, &mut outputs) {
            Ok(()) => Ok(outputs),
            Err(e) => Err(convert_error(e, ctx.data_mut())),
        };
    }

    let mut pending: Option<ResumableCallOutOfFuel> = None;
    // Fuel the next instruction needs (known after an out-of-fuel pause).
    let mut required: u64 = 1;
    loop {
        // Decide how much fuel to hand out for this slice.
        let (budget, deadline, slice) = {
            let d = ctx.data();
            (d.budget, d.deadline, d.slice)
        };
        let mut grant = match budget {
            Some(b) => b,
            None => u64::MAX / 4,
        };
        if deadline.is_some() {
            grant = grant.min(slice.max(required));
        }
        let slice_started = Instant::now();
        if grant < required || grant == 0 {
            return Err(OutOfFuel::new_err(format!(
                "fuel budget exhausted after {} units",
                ctx.data().consumed
            )));
        }
        ctx.set_fuel(grant).map_err(|e| WasmError::new_err(e.to_string()))?;

        let result = match pending.take() {
            None => func.call_resumable(&mut ctx, args, &mut outputs),
            Some(r) => r.resume(&mut ctx, &mut outputs),
        };

        // Account for consumed fuel.
        let left = ctx.get_fuel().unwrap_or(0);
        let used = grant.saturating_sub(left);
        {
            let d = ctx.data_mut();
            d.consumed = d.consumed.saturating_add(used);
            if let Some(b) = d.budget.as_mut() {
                *b = b.saturating_sub(used);
            }
        }

        match result {
            Ok(ResumableCall::Finished) => return Ok(outputs),
            Ok(ResumableCall::OutOfFuel(r)) => {
                required = r.required_fuel().max(1);
                // Adaptive slicing: fuel rates differ wildly between modules
                // (see notes on wasmi's per-block fuel accounting), so size
                // slices by wall time: aim for roughly 1-4 ms per slice.
                let elapsed = slice_started.elapsed();
                let d = ctx.data_mut();
                if elapsed < Duration::from_millis(1) {
                    d.slice = d.slice.saturating_mul(2).min(1 << 40);
                } else if elapsed > Duration::from_millis(4) {
                    d.slice = (d.slice / 2).max(1000);
                }
                let d = ctx.data();
                if d.budget.map_or(false, |b| b < required) {
                    return Err(OutOfFuel::new_err(format!(
                        "fuel budget exhausted after {} units",
                        d.consumed
                    )));
                }
                if let Some(dl) = d.deadline {
                    if Instant::now() >= dl {
                        return Err(Timeout::new_err("wall-clock deadline exceeded"));
                    }
                }
                pending = Some(r);
            }
            Ok(ResumableCall::HostTrap(r)) => {
                let err = r.into_host_error();
                return Err(convert_error(err, ctx.data_mut()));
            }
            Err(e) => {
                // Note: with lazy compilation, wasmi 2.0 charges fuel for
                // translating a function on first call and reports a shortfall
                // there as a plain (non-resumable) error even mid-execution.
                // That is why `Engine` defaults to eager compilation.
                return Err(convert_error(e, ctx.data_mut()));
            }
        }
    }
}

impl Store {
    /// Give `f` a store context: the active `Caller` if we are inside a host
    /// function of this store (re-entrant access), otherwise the store itself.
    fn with_ctx<R>(&self, f: impl FnOnce(StoreContextMut<'_, HostData>) -> PyResult<R>) -> PyResult<R> {
        let top = CALLERS.with(|c| c.borrow().last().copied());
        if let Some((id, ptr)) = top {
            if id == self.id {
                // SAFETY: the pointer was pushed by the host-function trampoline
                // of this store and is popped before that trampoline returns.
                // Guest execution is single-threaded and the GIL is held here.
                let caller = unsafe { &mut *ptr };
                return f(caller.as_context_mut());
            }
        }
        let mut store = self
            .store
            .try_borrow_mut()
            .map_err(|_| PyRuntimeError::new_err("store is busy (re-entrant use from another store's host function?)"))?;
        f(store.as_context_mut())
    }

    fn instance(&self) -> PyResult<WInstance> {
        self.instance
            .borrow()
            .ok_or_else(|| PyRuntimeError::new_err("store has no instance; call instantiate() first"))
    }

    fn export(&self, name: &str) -> PyResult<Extern> {
        let instance = self.instance()?;
        self.with_ctx(|ctx| {
            instance
                .get_export(&ctx, name)
                .ok_or_else(|| PyKeyError::new_err(format!("no export named {name:?}")))
        })
    }

    fn memory(&self, name: &str) -> PyResult<wasmi::Memory> {
        match self.export(name)? {
            Extern::Memory(m) => Ok(m),
            _ => Err(PyTypeError::new_err(format!("export {name:?} is not a memory"))),
        }
    }

    fn call_func(&self, py: Python<'_>, func: Func, args: &Bound<'_, PyAny>, timeout: Option<f64>) -> PyResult<Py<PyAny>> {
        let nested = CALLERS.with(|c| c.borrow().iter().any(|(id, _)| *id == self.id));
        if nested && native_stack_low() {
            return Err(Trap::new_err(("host native stack exhausted by nested guest calls", "StackOverflow")));
        }
        let results = self.with_ctx(|mut ctx| {
            let ty = func.ty(&ctx);
            let params = ty.params();
            let args: Vec<Bound<'_, PyAny>> = args.try_iter()?.collect::<PyResult<Vec<_>>>()?;
            if args.len() != params.len() {
                return Err(PyTypeError::new_err(format!(
                    "function expects {} arguments, got {}",
                    params.len(),
                    args.len()
                )));
            }
            let vals = args
                .iter()
                .zip(params.iter())
                .map(|(a, t)| py_to_val(a, t))
                .collect::<PyResult<Vec<_>>>()?;
            let previous_deadline = ctx.data().deadline;
            if !nested {
                ctx.data_mut().deadline = timeout.map(|t| Instant::now() + Duration::from_secs_f64(t));
            } else if let Some(t) = timeout {
                // Nested calls may only tighten the deadline.
                let candidate = Instant::now() + Duration::from_secs_f64(t);
                let d = ctx.data_mut();
                d.deadline = Some(match d.deadline {
                    Some(existing) => existing.min(candidate),
                    None => candidate,
                });
            }
            // Release the GIL while the interpreter runs: other Python threads
            // can proceed, and host functions re-attach on entry.
            let result = py.detach(|| run_call(ctx.as_context_mut(), func, &vals));
            if !nested {
                ctx.data_mut().deadline = None;
            } else {
                ctx.data_mut().deadline = previous_deadline;
            }
            result
        })?;
        results_to_py(py, &results)
    }
}

#[pymethods]
impl Store {
    /// Create a store with resource limits.
    ///
    /// max_memory_bytes: cap on each linear memory (growth beyond fails).
    /// trap_on_grow_failure: trap instead of returning -1 from memory.grow.
    /// fuel: initial fuel budget (None = unlimited).
    /// fuel_slice: fuel granted per slice while a timeout is active.
    #[new]
    #[pyo3(signature = (engine, max_memory_bytes = None, max_table_elements = None, max_instances = None, trap_on_grow_failure = false, fuel = None, fuel_slice = 250_000))]
    fn new(
        engine: &Engine,
        max_memory_bytes: Option<usize>,
        max_table_elements: Option<usize>,
        max_instances: Option<usize>,
        trap_on_grow_failure: bool,
        fuel: Option<u64>,
        fuel_slice: u64,
    ) -> PyResult<Self> {
        let mut builder = StoreLimitsBuilder::new().trap_on_grow_failure(trap_on_grow_failure);
        if let Some(m) = max_memory_bytes {
            builder = builder.memory_size(m);
        }
        if let Some(t) = max_table_elements {
            builder = builder.table_elements(t);
        }
        if let Some(i) = max_instances {
            builder = builder.instances(i);
        }
        let data = HostData {
            limits: builder.build(),
            funcs: Vec::new(),
            pending_err: None,
            budget: fuel,
            consumed: 0,
            deadline: None,
            slice: fuel_slice.max(1),
            fuel_enabled: engine.consume_fuel,
            invokes: 0,
            unwinds: 0,
            stack_guard_hits: 0,
        };
        let mut store = WStore::new(&engine.inner, data);
        store.limiter(|data| &mut data.limits);
        let linker = Linker::new(&engine.inner);
        Ok(Store {
            id: STORE_IDS.fetch_add(1, Ordering::Relaxed),
            store: RefCell::new(store),
            linker: RefCell::new(linker),
            instance: RefCell::new(None),
            fuel_enabled: engine.consume_fuel,
        })
    }

    /// Remaining fuel budget, or None if unlimited.
    #[getter]
    fn get_fuel(&self) -> PyResult<Option<u64>> {
        self.with_ctx(|ctx| Ok(ctx.data().budget))
    }

    #[setter]
    fn set_fuel(&self, fuel: Option<u64>) -> PyResult<()> {
        self.with_ctx(|mut ctx| {
            ctx.data_mut().budget = fuel;
            Ok(())
        })
    }

    /// Total fuel consumed by all calls so far.
    #[getter]
    fn fuel_consumed(&self) -> PyResult<u64> {
        self.with_ctx(|ctx| Ok(ctx.data().consumed))
    }

    /// Register a Python callable as host function `module.name` with the given
    /// wasm signature (lists of "i32"/"i64"/"f32"/"f64").
    fn define_func(
        &self,
        module: &str,
        name: &str,
        params: Vec<String>,
        results: Vec<String>,
        callable: Py<PyAny>,
    ) -> PyResult<()> {
        let params = params.iter().map(|s| parse_valtype(s)).collect::<PyResult<Vec<_>>>()?;
        let results = results.iter().map(|s| parse_valtype(s)).collect::<PyResult<Vec<_>>>()?;
        let ty = FuncType::new(params, results.clone());
        let idx = {
            let mut store = self.store.try_borrow_mut().map_err(|_| PyRuntimeError::new_err("store is busy"))?;
            let data = store.data_mut();
            data.funcs.push(callable);
            data.funcs.len() - 1
        };
        let store_id = self.id;
        let n_results = results.len();
        let mut linker = self.linker.try_borrow_mut().map_err(|_| PyRuntimeError::new_err("linker is busy"))?;
        linker
            .func_new(module, name, ty, move |mut caller: Caller<'_, HostData>, params: &[Val], out: &mut [Val]| {
                Python::attach(|py| {
                    // Wall-clock check at every host call boundary too.
                    if let Some(dl) = caller.data().deadline {
                        if Instant::now() >= dl {
                            caller.data_mut().pending_err = Some(Timeout::new_err("wall-clock deadline exceeded"));
                            return Err(WError::host(PyHostError));
                        }
                    }
                    let f = caller.data().funcs[idx].clone_ref(py);
                    let args = params
                        .iter()
                        .map(|v| val_to_py(py, v))
                        .collect::<PyResult<Vec<_>>>();
                    let args = match args {
                        Ok(a) => a,
                        Err(e) => {
                            caller.data_mut().pending_err = Some(e);
                            return Err(WError::host(PyHostError));
                        }
                    };
                    let ptr = &mut caller as *mut Caller<'_, HostData> as *mut Caller<'static, HostData>;
                    CALLERS.with(|c| c.borrow_mut().push((store_id, ptr)));
                    let res = PyTuple::new(py, args).and_then(|t| f.call1(py, t));
                    CALLERS.with(|c| c.borrow_mut().pop());
                    let ret = match res {
                        Ok(r) => r,
                        Err(e) => {
                            caller.data_mut().pending_err = Some(e);
                            return Err(WError::host(PyHostError));
                        }
                    };
                    let ret = ret.bind(py);
                    let conv: PyResult<()> = (|| {
                        match n_results {
                            0 => {}
                            1 => {
                                let ty = out[0].ty();
                                out[0] = py_to_val(ret, &ty)?;
                            }
                            n => {
                                let items: Vec<Bound<'_, PyAny>> = ret.try_iter()?.collect::<PyResult<Vec<_>>>()?;
                                if items.len() != n {
                                    return Err(PyTypeError::new_err(format!(
                                        "host function must return {n} values, got {}",
                                        items.len()
                                    )));
                                }
                                for (i, item) in items.iter().enumerate() {
                                    let ty = out[i].ty();
                                    out[i] = py_to_val(item, &ty)?;
                                }
                            }
                        }
                        Ok(())
                    })();
                    if let Err(e) = conv {
                        caller.data_mut().pending_err = Some(e);
                        return Err(WError::host(PyHostError));
                    }
                    Ok(())
                })
            })
            .map_err(|e| LinkError::new_err(e.to_string()))?;
        Ok(())
    }

    /// Register `module.name` as the emscripten-style `_emscripten_throw_longjmp`
    /// import: calling it unwinds the guest stack up to the nearest native invoke.
    fn define_longjmp_thrower(&self, module: &str, name: &str) -> PyResult<()> {
        let mut linker = self.linker.try_borrow_mut().map_err(|_| PyRuntimeError::new_err("linker is busy"))?;
        linker
            .func_new(module, name, FuncType::new([], []), move |mut caller: Caller<'_, HostData>, _params: &[Val], _out: &mut [Val]| {
                caller.data_mut().pending_err = Some(LongjmpUnwind::new_err(()));
                caller.data_mut().unwinds += 1;
                Err(WError::host(PyHostError))
            })
            .map_err(|e| LinkError::new_err(e.to_string()))?;
        Ok(())
    }

    /// Register `module.name` as an emscripten-style `invoke_*` trampoline
    /// implemented natively: it calls table[index](args...) and, if a longjmp
    /// unwinds through it, restores the shadow stack pointer (global
    /// `stack_pointer_global` or export `stack_restore`), calls `setThrew(1, 0)`
    /// and returns zero.
    #[pyo3(signature = (module, name, params, results, table = "__indirect_function_table", stack_pointer_global = "__stack_pointer", overflow_export = None))]
    fn define_invoke(
        &self,
        module: &str,
        name: &str,
        params: Vec<String>,
        results: Vec<String>,
        table: &str,
        stack_pointer_global: &str,
        overflow_export: Option<String>,
    ) -> PyResult<()> {
        let params = params.iter().map(|s| parse_valtype(s)).collect::<PyResult<Vec<_>>>()?;
        let results = results.iter().map(|s| parse_valtype(s)).collect::<PyResult<Vec<_>>>()?;
        if params.first() != Some(&ValType::I32) {
            return Err(PyValueError::new_err("invoke_* functions take the table index as first i32 parameter"));
        }
        let ty = FuncType::new(params, results);
        let table_name = table.to_string();
        let sp_name = stack_pointer_global.to_string();
        let mut linker = self.linker.try_borrow_mut().map_err(|_| PyRuntimeError::new_err("linker is busy"))?;
        linker
            .func_new(module, name, ty, move |mut caller: Caller<'_, HostData>, params: &[Val], out: &mut [Val]| {
                caller.data_mut().invokes += 1;
                let index = match params[0] {
                    Val::I32(i) => i as u32 as u64,
                    _ => unreachable!(),
                };
                let table = match caller.get_export(&table_name) {
                    Some(Extern::Table(t)) => t,
                    _ => {
                        caller.data_mut().pending_err = Some(PyRuntimeError::new_err(format!("no exported table {table_name:?}")));
                        return Err(WError::host(PyHostError));
                    }
                };
                let mut func = match table.get(&caller, index) {
                    Some(Ref::Func(wasmi::Nullable::Val(f))) => f,
                    _ => return Err(WError::from(TrapCode::IndirectCallToNull)),
                };
                let mut call_args: &[Val] = &params[1..];
                // Host native stack guard: instead of the requested function,
                // call the guest's overflow handler (which raises a guest-level
                // exception via longjmp), or trap if there is none.
                if native_stack_low() {
                    caller.data_mut().stack_guard_hits += 1;
                    match overflow_export.as_deref().and_then(|n| caller.get_export(n)) {
                        Some(Extern::Func(f)) => {
                            func = f;
                            call_args = &[];
                        }
                        _ => {
                            caller.data_mut().pending_err = Some(Trap::new_err((
                                "host native stack exhausted by nested guest calls",
                                "StackOverflow",
                            )));
                            return Err(WError::host(PyHostError));
                        }
                    }
                }
                // Save the shadow stack pointer.
                let sp_global = match caller.get_export(&sp_name) {
                    Some(Extern::Global(g)) => Some(g),
                    _ => None,
                };
                let saved_sp = match sp_global {
                    Some(g) => Some(g.get(&caller)),
                    None => match caller.get_export("stack_save") {
                        Some(Extern::Func(f)) => {
                            let mut r = [Val::I32(0)];
                            f.call(&mut caller, &[], &mut r)?;
                            Some(r[0].clone())
                        }
                        _ => None,
                    },
                };
                match run_call(caller.as_context_mut(), func, call_args) {
                    Ok(vals) => {
                        for (o, v) in out.iter_mut().zip(vals) {
                            *o = v;
                        }
                        Ok(())
                    }
                    Err(err) => {
                        let is_unwind = Python::attach(|py| err.is_instance_of::<LongjmpUnwind>(py));
                        if !is_unwind {
                            caller.data_mut().pending_err = Some(err);
                            return Err(WError::host(PyHostError));
                        }
                        // Restore the stack pointer and flag the unwind for the guest.
                        if let (Some(g), Some(sp)) = (sp_global, saved_sp.clone()) {
                            g.set(&mut caller, sp).map_err(|e| WError::new(e.to_string()))?;
                        } else if let (Some(Extern::Func(f)), Some(sp)) = (caller.get_export("stack_restore"), saved_sp) {
                            f.call(&mut caller, &[sp], &mut [])?;
                        }
                        if let Some(Extern::Func(f)) = caller.get_export("setThrew") {
                            f.call(&mut caller, &[Val::I32(1), Val::I32(0)], &mut [])?;
                        }
                        for o in out.iter_mut() {
                            *o = default_val(&o.ty());
                        }
                        Ok(())
                    }
                }
            })
            .map_err(|e| LinkError::new_err(e.to_string()))?;
        Ok(())
    }

    /// Counters for the native setjmp/longjmp support: (invokes, unwinds, stack guard hits).
    #[getter]
    fn sjlj_stats(&self) -> PyResult<(u64, u64, u64)> {
        self.with_ctx(|ctx| Ok((ctx.data().invokes, ctx.data().unwinds, ctx.data().stack_guard_hits)))
    }

    /// Instantiate `module` (running its start function) and make it the store's instance.
    fn instantiate(&self, py: Python<'_>, module: &Module) -> PyResult<()> {
        let mut store = self.store.try_borrow_mut().map_err(|_| PyRuntimeError::new_err("store is busy"))?;
        let linker = self.linker.borrow();
        // Give the start function a fuel allowance from the budget (no slicing).
        let budget = store.data().budget;
        if store.data().fuel_enabled {
            let grant = budget.unwrap_or(u64::MAX / 4);
            store.set_fuel(grant).map_err(|e| WasmError::new_err(e.to_string()))?;
        }
        let result = {
            // RefCell guards are not Send; plain references to the (Send + Sync)
            // wasmi objects are, so detach with those.
            let store_ref: &mut WStore<HostData> = &mut store;
            let linker_ref: &Linker<HostData> = &linker;
            let module_ref: &WModule = &module.inner;
            py.detach(|| linker_ref.instantiate_and_start(store_ref, module_ref))
        };
        if store.data().fuel_enabled {
            let grant = budget.unwrap_or(u64::MAX / 4);
            let left = store.get_fuel().unwrap_or(0);
            let used = grant.saturating_sub(left);
            let d = store.data_mut();
            d.consumed += used;
            if let Some(b) = d.budget.as_mut() {
                *b = b.saturating_sub(used);
            }
        }
        match result {
            Ok(instance) => {
                *self.instance.borrow_mut() = Some(instance);
                Ok(())
            }
            Err(e) => Err(convert_error(e, store.data_mut())),
        }
    }

    /// Names and kinds of the instance's exports.
    fn exports(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let instance = self.instance()?;
        self.with_ctx(|ctx| {
            let list = PyList::empty(py);
            for export in instance.exports(&ctx) {
                let kind = match export.ty(&ctx) {
                    ExternType::Func(_) => "func",
                    ExternType::Memory(_) => "memory",
                    ExternType::Table(_) => "table",
                    ExternType::Global(_) => "global",
                };
                list.append((export.name(), kind))?;
            }
            Ok(list.unbind())
        })
    }

    /// Call exported function `name` with `args` (a sequence).
    ///
    /// timeout: wall-clock seconds for this call (requires fuel metering).
    #[pyo3(signature = (name, args = None, timeout = None))]
    fn call(&self, py: Python<'_>, name: &str, args: Option<&Bound<'_, PyAny>>, timeout: Option<f64>) -> PyResult<Py<PyAny>> {
        let func = match self.export(name)? {
            Extern::Func(f) => f,
            _ => return Err(PyTypeError::new_err(format!("export {name:?} is not a function"))),
        };
        let empty = PyTuple::empty(py);
        let args = args.cloned().unwrap_or_else(|| empty.into_any());
        self.call_func(py, func, &args, timeout)
    }

    /// Call the function at `index` in the exported table `table` (call_indirect from the host).
    #[pyo3(signature = (table, index, args = None, timeout = None))]
    fn call_indirect(&self, py: Python<'_>, table: &str, index: u64, args: Option<&Bound<'_, PyAny>>, timeout: Option<f64>) -> PyResult<Py<PyAny>> {
        let table = match self.export(table)? {
            Extern::Table(t) => t,
            _ => return Err(PyTypeError::new_err(format!("export {table:?} is not a table"))),
        };
        let func = self.with_ctx(|ctx| match table.get(&ctx, index) {
            Some(Ref::Func(wasmi::Nullable::Val(f))) => Ok(f),
            Some(Ref::Func(wasmi::Nullable::Null)) => Err(Trap::new_err(("null function reference in table", "IndirectCallToNull"))),
            Some(_) => Err(PyTypeError::new_err("table element is not a funcref")),
            None => Err(Trap::new_err(("table index out of bounds", "TableOutOfBounds"))),
        })?;
        let empty = PyTuple::empty(py);
        let args = args.cloned().unwrap_or_else(|| empty.into_any());
        self.call_func(py, func, &args, timeout)
    }

    /// Signature of an exported function as (params, results) lists.
    fn func_type(&self, name: &str) -> PyResult<(Vec<&'static str>, Vec<&'static str>)> {
        let func = match self.export(name)? {
            Extern::Func(f) => f,
            _ => return Err(PyTypeError::new_err(format!("export {name:?} is not a function"))),
        };
        self.with_ctx(|ctx| {
            let ty = func.ty(&ctx);
            Ok((
                ty.params().iter().map(valtype_name).collect(),
                ty.results().iter().map(valtype_name).collect(),
            ))
        })
    }

    /// Size in bytes of the exported memory.
    #[pyo3(signature = (name = "memory"))]
    fn memory_size(&self, name: &str) -> PyResult<usize> {
        let mem = self.memory(name)?;
        self.with_ctx(|ctx| Ok(mem.data_size(&ctx)))
    }

    /// Grow the exported memory by `pages` (64 KiB each); returns the old size in pages or -1.
    #[pyo3(signature = (pages, name = "memory"))]
    fn memory_grow(&self, pages: u64, name: &str) -> PyResult<i64> {
        let mem = self.memory(name)?;
        self.with_ctx(|mut ctx| match mem.grow(&mut ctx, pages) {
            Ok(old) => Ok(old as i64),
            Err(_) => Ok(-1),
        })
    }

    /// Read `length` bytes at `offset` from the exported memory.
    #[pyo3(signature = (offset, length, name = "memory"))]
    fn memory_read<'py>(&self, py: Python<'py>, offset: usize, length: usize, name: &str) -> PyResult<Bound<'py, PyBytes>> {
        let mem = self.memory(name)?;
        self.with_ctx(|ctx| {
            let data = mem.data(&ctx);
            let end = offset.checked_add(length).ok_or_else(|| PyValueError::new_err("overflow"))?;
            if end > data.len() {
                return Err(Trap::new_err((
                    format!("host memory read out of bounds: {offset}+{length} > {}", data.len()),
                    "MemoryOutOfBounds",
                )));
            }
            Ok(PyBytes::new(py, &data[offset..end]))
        })
    }

    /// Write `data` at `offset` into the exported memory.
    #[pyo3(signature = (offset, data, name = "memory"))]
    fn memory_write(&self, offset: usize, data: &[u8], name: &str) -> PyResult<()> {
        let mem = self.memory(name)?;
        self.with_ctx(|mut ctx| {
            let buf = mem.data_mut(&mut ctx);
            let end = offset.checked_add(data.len()).ok_or_else(|| PyValueError::new_err("overflow"))?;
            if end > buf.len() {
                return Err(Trap::new_err((
                    format!("host memory write out of bounds: {offset}+{} > {}", data.len(), buf.len()),
                    "MemoryOutOfBounds",
                )));
            }
            buf[offset..end].copy_from_slice(data);
            Ok(())
        })
    }

    /// Read a NUL-terminated string from memory.
    #[pyo3(signature = (offset, max_len = 1 << 20, name = "memory"))]
    fn memory_read_cstr<'py>(&self, py: Python<'py>, offset: usize, max_len: usize, name: &str) -> PyResult<Bound<'py, PyBytes>> {
        let mem = self.memory(name)?;
        self.with_ctx(|ctx| {
            let data = mem.data(&ctx);
            if offset > data.len() {
                return Err(Trap::new_err(("host memory read out of bounds", "MemoryOutOfBounds")));
            }
            let slice = &data[offset..data.len().min(offset + max_len)];
            let n = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
            Ok(PyBytes::new(py, &slice[..n]))
        })
    }

    /// Get the value of an exported global.
    fn global_get(&self, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        let g = match self.export(name)? {
            Extern::Global(g) => g,
            _ => return Err(PyTypeError::new_err(format!("export {name:?} is not a global"))),
        };
        self.with_ctx(|ctx| val_to_py(py, &g.get(&ctx)))
    }

    /// Set the value of an exported (mutable) global.
    fn global_set(&self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let g = match self.export(name)? {
            Extern::Global(g) => g,
            _ => return Err(PyTypeError::new_err(format!("export {name:?} is not a global"))),
        };
        self.with_ctx(|mut ctx| {
            let ty = g.ty(&ctx);
            let val = py_to_val(value, &ty.content())?;
            g.set(&mut ctx, val).map_err(|e| WasmError::new_err(e.to_string()))
        })
    }

    /// True while a host function of this store is executing (re-entrant context).
    #[getter]
    fn in_host_call(&self) -> bool {
        CALLERS.with(|c| c.borrow().iter().any(|(id, _)| *id == self.id))
    }
}

/// Convert WebAssembly text format to binary.
#[pyfunction]
fn wat2wasm(py: Python<'_>, text: &str) -> PyResult<Py<PyBytes>> {
    let bytes = wat::parse_str(text).map_err(|e| WasmError::new_err(e.to_string()))?;
    Ok(PyBytes::new(py, &bytes).unbind())
}

/// Test helper: sleep with the GIL released (verifies detach works).
#[pyfunction]
fn _sleep_detached(py: Python<'_>, seconds: f64) {
    py.detach(|| std::thread::sleep(Duration::from_secs_f64(seconds)));
}

#[pymodule]
fn _core(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(wat2wasm, m)?)?;
    m.add_function(wrap_pyfunction!(_sleep_detached, m)?)?;
    m.add_class::<Engine>()?;
    m.add_class::<Module>()?;
    m.add_class::<Store>()?;
    m.add("WasmError", py.get_type::<WasmError>())?;
    m.add("Trap", py.get_type::<Trap>())?;
    m.add("OutOfFuel", py.get_type::<OutOfFuel>())?;
    m.add("Timeout", py.get_type::<Timeout>())?;
    m.add("Exit", py.get_type::<Exit>())?;
    m.add("LinkError", py.get_type::<LinkError>())?;
    m.add("LongjmpUnwind", py.get_type::<LongjmpUnwind>())?;
    m.add("WASMI_VERSION", "2.0.0")?;
    Ok(())
}
