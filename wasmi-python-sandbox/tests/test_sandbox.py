import time

import pytest

import wasmi_sandbox as ws

WAT = """
(module
  (import "env" "add" (func $add (param i32 i32) (result i32)))
  (import "env" "log" (func $log (param i32)))
  (memory (export "memory") 1 100)
  (table (export "tbl") 2 funcref)
  (elem (i32.const 0) $double $loop)
  (func $double (export "double") (param i32) (result i32)
    (call $log (local.get 0))
    (call $add (local.get 0) (local.get 0)))
  (func $loop (export "loop") (param i32) (result i32)
    (local i32)
    (block (loop (br_if 1 (i32.eqz (local.get 0)))
      (local.set 0 (i32.sub (local.get 0) (i32.const 1)))
      (local.set 1 (i32.add (local.get 1) (i32.const 1)))
      (br 0)))
    (local.get 1))
  (func (export "forever") (loop (br 0)))
  (func (export "grow") (param i32) (result i32) (memory.grow (local.get 0)))
  (func (export "boom") (unreachable))
  (func (export "mixed") (param i64 f32 f64) (result i64 f64)
    (i64.add (local.get 0) (i64.const 1))
    (f64.add (f64.promote_f32 (local.get 1)) (local.get 2)))
)
"""


@pytest.fixture(scope="module")
def engine():
    return ws.Engine()


@pytest.fixture(scope="module")
def module(engine):
    return ws.Module(engine, ws.wat2wasm(WAT))


def make_store(engine, module, **kw):
    logged = []
    store = ws.Store(engine, **kw)
    store.define_func("env", "add", ["i32", "i32"], ["i32"], lambda a, b: a + b)
    store.define_func("env", "log", ["i32"], [], logged.append)
    store.instantiate(module)
    return store, logged


def test_module_introspection(module):
    imports = module.imports()
    assert [(i["module"], i["name"], i["params"], i["results"]) for i in imports] == [
        ("env", "add", ["i32", "i32"], ["i32"]),
        ("env", "log", ["i32"], []),
    ]
    names = {e["name"]: e["kind"] for e in module.exports()}
    assert names["memory"] == "memory" and names["double"] == "func" and names["tbl"] == "table"


def test_call_and_host_functions(engine, module):
    store, logged = make_store(engine, module)
    assert store.call("double", [21]) == 42
    assert logged == [21]
    assert store.call("mixed", [5, 1.5, 2.25]) == (6, 3.75)
    assert store.func_type("mixed") == (["i64", "f32", "f64"], ["i64", "f64"])


def test_memory_limit(engine, module):
    store, _ = make_store(engine, module, max_memory_bytes=10 * 65536)
    assert store.memory_size() == 65536
    assert store.call("grow", [5]) == 1  # old size in pages
    assert store.call("grow", [20]) == -1  # refused by the limiter
    assert store.memory_size() == 6 * 65536
    store.memory_write(0, b"hello")
    assert store.memory_read(0, 5) == b"hello"
    with pytest.raises(ws.Trap):
        store.memory_read(store.memory_size() - 2, 4)


def test_memory_limit_trapping(engine, module):
    store, _ = make_store(engine, module, max_memory_bytes=2 * 65536, trap_on_grow_failure=True)
    with pytest.raises(ws.Trap):
        store.call("grow", [20])


def test_trap(engine, module):
    store, _ = make_store(engine, module)
    with pytest.raises(ws.Trap) as ei:
        store.call("boom")
    assert ei.value.args[1] == "UnreachableCodeReached"


def test_fuel_budget(engine, module):
    store, _ = make_store(engine, module, fuel=100_000)
    with pytest.raises(ws.OutOfFuel):
        store.call("loop", [1_000_000])
    assert store.fuel is not None and store.fuel < 100_000
    assert store.fuel_consumed > 0
    store.fuel = 50_000_000
    assert store.call("loop", [1_000_000]) == 1_000_000
    assert store.fuel < 50_000_000


def test_timeout(engine, module):
    store, _ = make_store(engine, module)
    t = time.time()
    with pytest.raises(ws.Timeout):
        store.call("forever", timeout=0.3)
    assert 0.25 < time.time() - t < 2
    # Store is still usable afterwards.
    assert store.call("double", [2]) == 4
    assert store.call("loop", [100_000], timeout=5) == 100_000


def test_reentrancy(engine, module):
    store = ws.Store(engine)
    store.define_func("env", "add", ["i32", "i32"], ["i32"], lambda a, b: a + store.call_indirect("tbl", 1, [10]))
    store.define_func("env", "log", ["i32"], [], lambda v: None)
    store.instantiate(module)
    assert store.call("double", [1]) == 11


def test_host_exception_propagates(engine, module):
    class Boom(Exception):
        pass

    def bad(a, b):
        raise Boom("from host")

    store = ws.Store(engine)
    store.define_func("env", "add", ["i32", "i32"], ["i32"], bad)
    store.define_func("env", "log", ["i32"], [], lambda v: None)
    store.instantiate(module)
    with pytest.raises(Boom):
        store.call("double", [1])
    # and the store keeps working
    assert store.call("loop", [3]) == 3


def test_link_error(engine, module):
    store = ws.Store(engine)
    with pytest.raises(ws.LinkError):
        store.instantiate(module)


def test_sandbox_wrapper(engine):
    sb = ws.Sandbox(ws.wat2wasm(WAT), engine=engine, imports={"env": {"add": lambda a, b: a * b, "log": lambda v: None}}, max_memory=1 << 20, timeout=2)
    assert sb.call("double", 7) == 49
    assert sb.memory_size == 65536
    with pytest.raises(ws.Timeout):
        sb.call("forever")


def test_wasi_proc_exit():
    wat = """
    (module
      (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
      (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 8) "hi\\n")
      (func (export "_start")
        (i32.store (i32.const 0) (i32.const 8))
        (i32.store (i32.const 4) (i32.const 3))
        (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 16)))
        (call $exit (i32.const 7))))
    """
    sb = ws.Sandbox(ws.wat2wasm(wat))
    with pytest.raises(ws.Exit) as ei:
        sb.call("_start")
    assert ei.value.args[0] == 7
    assert sb.stdout == "hi\n"
