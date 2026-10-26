"""Demo: wasmi's C API from Python via ctypes.

Usage: LIBWASMI=/path/to/libwasmi.so python3 demo.py
"""

import ctypes as C
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from wasmi_capi import CapiSandbox, load, wasm_byte_vec_t  # noqa: E402

# A tiny module: imports env.add/env.log, exports double(), loop(), memory.
# (Binary produced from WAT with wasmi itself; see make_wasm() below.)
WAT = b"""
(module
  (import "env" "add" (func $add (param i32 i32) (result i32)))
  (import "env" "log" (func $log (param i32)))
  (memory (export "memory") 1 4)
  (data (i32.const 0) "hi from wasm")
  (func (export "double") (param i32) (result i32)
    (call $log (local.get 0))
    (call $add (local.get 0) (local.get 0)))
  (func (export "loop") (param i32) (result i32)
    (local i32)
    (block (loop (br_if 1 (i32.eqz (local.get 0)))
      (local.set 0 (i32.sub (local.get 0) (i32.const 1)))
      (local.set 1 (i32.add (local.get 1) (i32.const 1)))
      (br 0)))
    (local.get 1))
)
"""


def make_wasm() -> bytes:
    """wasm.h has no text-format parser; use the PyO3 package (or any wat tool)."""
    try:
        import wasmi_sandbox  # noqa: F401
    except ImportError:
        raise SystemExit("need the wasmi_sandbox package (or a .wasm file) to produce the binary")
    # wasmi's Module accepts .wat; but we need bytes for the C API, so use `wat`
    # via a quick round trip: compile to check validity, then hand-assemble is
    # overkill. Instead we ship the binary alongside this script.
    here = os.path.dirname(__file__)
    return open(os.path.join(here, "demo.wasm"), "rb").read()


def main():
    lib = load()
    wasm = make_wasm()
    logged = []
    sb = CapiSandbox(lib, wasm, {"env": {"add": lambda a, b: a + b, "log": logged.append}}, fuel=None)
    print("exports:", sorted(sb.exports))
    print("double(21) =", sb.call("double", 21), "| logged:", logged)
    print("memory[0:12] =", sb.read(0, 12))
    t = time.time()
    print("loop(1e6) =", sb.call("loop", 1_000_000), "in %.3fs" % (time.time() - t))

    def boom(a, b):
        raise ValueError("host says no")

    sb2 = CapiSandbox(lib, wasm, {"env": {"add": boom, "log": lambda v: None}})
    try:
        sb2.call("double", 1)
    except ValueError as e:
        print("host exception propagated through wasm_trap:", e)

    # Fuel metering via wasmi.h: needs a wasmi_store_t rather than wasm_store_t,
    # so demonstrate the raw calls.
    config = lib.wasm_config_new()
    lib.wasmi_config_consume_fuel_set(config, True)
    engine = lib.wasm_engine_new_with_config(config)
    store = lib.wasmi_store_new(engine, None, None)
    ctx = lib.wasmi_store_context(store)
    err = lib.wasmi_context_set_fuel(ctx, 1000)
    if err:
        lib.wasmi_error_delete(err)
    fuel = C.c_uint64()
    err2 = lib.wasmi_context_get_fuel(ctx, C.byref(fuel))
    if err2:
        lib.wasmi_error_delete(err2)
    print("wasmi_context_set_fuel(1000) ->", "ok" if not err else "error", "| get_fuel ->", fuel.value)
    # Instantiating through a wasmi_store_t requires the wasmi_* extern/func
    # API (mirroring wasmtime's), which the current C API only partially
    # exposes; see README for the discussion.
    lib.wasmi_store_delete(store)
    lib.wasm_engine_delete(engine)

    n = 20000
    t = time.time()
    for i in range(n):
        sb.call("double", i)
    print("host round trip via ctypes: %.1f us per call" % ((time.time() - t) / n * 1e6))


if __name__ == "__main__":
    main()
