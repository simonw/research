"""Compare wasmi 2.0 (this package) with wasmtime-py on the same guests.

Workloads:
  * raw wasm loop (10M iterations) - interpreter vs JIT
  * QuickJS fib(27) inside the guest
  * host-call round trips (guest -> Python -> guest)
Run: PYTHONPATH=../wasmi_sandbox/python python3 bench.py
"""

import struct
import time

import wasmi_sandbox as ws

try:
    import wasmtime
except ImportError:  # pragma: no cover
    wasmtime = None

LOOP_WAT = """
(module
  (import "env" "add" (func $add (param i32 i32) (result i32)))
  (func (export "loop") (param i32) (result i32)
    (local i32)
    (block (loop (br_if 1 (i32.eqz (local.get 0)))
      (local.set 0 (i32.sub (local.get 0) (i32.const 1)))
      (local.set 1 (i32.add (local.get 1) (i32.const 1)))
      (br 0)))
    (local.get 1))
  (func (export "hostloop") (param i32) (result i32)
    (local i32)
    (block (loop (br_if 1 (i32.eqz (local.get 0)))
      (local.set 0 (i32.sub (local.get 0) (i32.const 1)))
      (local.set 1 (call $add (local.get 1) (i32.const 1)))
      (br 0)))
    (local.get 1))
)
"""

FIB_JS = "function fib(n){return n<2?n:fib(n-1)+fib(n-2)}; fib(27)"


def timed(label, fn):
    t = time.perf_counter()
    r = fn()
    dt = time.perf_counter() - t
    print(f"{label:58} {dt*1000:9.1f} ms   -> {r}")
    return dt


def bench_wasmi():
    print("== wasmi 2.0 via wasmi_sandbox (interpreter, fuel metering on)")
    wasm = ws.wat2wasm(LOOP_WAT)
    sb = ws.Sandbox(wasm, imports={"env": {"add": lambda a, b: a + b}}, wasi=False)
    timed("wasm loop 10M iterations", lambda: sb.call("loop", 10_000_000))
    timed("wasm loop 10M iterations with 10s timeout (fuel slicing)", lambda: sb.call("loop", 10_000_000, timeout=10))
    timed("host-call loop 1M iterations (guest->Python->guest)", lambda: sb.call("hostloop", 1_000_000))
    eng_nofuel = ws.Engine(consume_fuel=False)
    sb2 = ws.Sandbox(wasm, engine=eng_nofuel, imports={"env": {"add": lambda a, b: a + b}}, wasi=False)
    timed("wasm loop 10M iterations, fuel metering OFF", lambda: sb2.call("loop", 10_000_000))

    t = time.perf_counter()
    js = ws.QuickJS()
    print(f"{'QuickJS init (eager compile of 1.3 MB module + qjs_init)':58} {(time.perf_counter()-t)*1000:9.1f} ms")
    timed("QuickJS fib(27)", lambda: js.eval(FIB_JS))
    timed("QuickJS fib(27) with 10s timeout", lambda: js.eval(FIB_JS, timeout=10))
    timed("QuickJS 20k host.add() calls", lambda: js.eval("let s=0; for(let i=0;i<20000;i++) s=host.add(s,1); s") if js.register("add", lambda a, b: a + b) is None else None)

    t = time.perf_counter()
    mp = ws.MicroPython()
    print(f"{'MicroPython init':58} {(time.perf_counter()-t)*1000:9.1f} ms")
    timed("MicroPython fib(20)", lambda: mp.exec("def fib(n):\n    return n if n < 2 else fib(n-1) + fib(n-2)\nprint(fib(20))").strip())
    timed("MicroPython 100k-iteration loop", lambda: mp.exec("s = 0\nfor i in range(100000):\n    s += i\nprint(s)").strip())
    print(f"   (MicroPython invoke_* trampolines so far: {mp.sjlj.invokes:,}, unwinds: {mp.sjlj.unwinds})")


def bench_wasmtime():
    if wasmtime is None:
        print("wasmtime-py not installed")
        return
    print("\n== wasmtime-py (Cranelift JIT)")
    engine = wasmtime.Engine()
    module = wasmtime.Module(engine, LOOP_WAT)
    store = wasmtime.Store(engine)
    add = wasmtime.Func(store, wasmtime.FuncType([wasmtime.ValType.i32(), wasmtime.ValType.i32()], [wasmtime.ValType.i32()]), lambda a, b: a + b)
    inst = wasmtime.Instance(store, module, [add])
    loop = inst.exports(store)["loop"]
    hostloop = inst.exports(store)["hostloop"]
    timed("wasm loop 10M iterations", lambda: loop(store, 10_000_000))
    timed("host-call loop 1M iterations (guest->Python->guest)", lambda: hostloop(store, 1_000_000))

    cfg = wasmtime.Config()
    cfg.consume_fuel = True
    engine2 = wasmtime.Engine(cfg)
    module2 = wasmtime.Module(engine2, LOOP_WAT)
    store2 = wasmtime.Store(engine2)
    store2.set_fuel(10**12)
    add2 = wasmtime.Func(store2, wasmtime.FuncType([wasmtime.ValType.i32(), wasmtime.ValType.i32()], [wasmtime.ValType.i32()]), lambda a, b: a + b)
    inst2 = wasmtime.Instance(store2, module2, [add2])
    timed("wasm loop 10M iterations, fuel metering ON", lambda: inst2.exports(store2)["loop"](store2, 10_000_000))

    # QuickJS guest under wasmtime with a minimal WASI + our env imports.
    t = time.perf_counter()
    qjs_bytes = ws.guest_path("quickjs.wasm").read_bytes()
    module3 = wasmtime.Module(engine, qjs_bytes)
    print(f"{'QuickJS module compile (Cranelift)':58} {(time.perf_counter()-t)*1000:9.1f} ms")
    store3 = wasmtime.Store(engine)
    linker = wasmtime.Linker(engine)
    wasi = wasmtime.WasiConfig()
    wasi.inherit_stdout()
    store3.set_wasi(wasi)
    linker.define_wasi()
    result = bytearray()
    mem_holder = {}

    def host_write(fd, ptr, length):
        if fd == 3:
            result.extend(mem_holder["mem"].read(store3, ptr, ptr + length))

    def host_call(*a):
        return 0

    def host_take(*a):
        return None

    i32 = wasmtime.ValType.i32()
    linker.define(store3, "env", "host_write", wasmtime.Func(store3, wasmtime.FuncType([i32, i32, i32], []), host_write))
    linker.define(store3, "env", "host_call", wasmtime.Func(store3, wasmtime.FuncType([i32, i32, i32, i32], [i32]), host_call))
    linker.define(store3, "env", "host_take", wasmtime.Func(store3, wasmtime.FuncType([i32, i32], []), host_take))
    linker.define(store3, "env", "host_interrupt", wasmtime.Func(store3, wasmtime.FuncType([], [i32]), lambda: 0))
    inst3 = linker.instantiate(store3, module3)
    ex = inst3.exports(store3)
    mem_holder["mem"] = ex["memory"]
    ex["_initialize"](store3)
    ex["qjs_init"](store3, 0, 0)
    code = FIB_JS.encode() + b"\0"
    ptr = ex["malloc"](store3, len(code))
    ex["memory"].write(store3, code, ptr)

    def run():
        result.clear()
        ex["qjs_eval"](store3, ptr, len(code) - 1)
        return bytes(result).decode()

    timed("QuickJS fib(27)", run)


if __name__ == "__main__":
    bench_wasmi()
    bench_wasmtime()
