"""Run untrusted Python in MicroPython compiled to WebAssembly.

MicroPython relies on setjmp/longjmp for exceptions. wasmi has no Wasm
exception handling, so the guest is compiled with LLVM's emscripten-style
lowering and the *host* provides the `invoke_*` trampolines: each one calls
back into the guest through its function table and catches the unwind.
"""

from __future__ import annotations

import json
from typing import Any, Callable, Dict, Optional

from ._core import Engine
from .sandbox import NativeInvoke, NativeLongjmp, Sandbox, guest_path, _NO_TIMEOUT



class PythonError(Exception):
    """An uncaught exception inside the MicroPython guest (traceback text)."""


class _LongjmpUnwind(Exception):
    """Internal: raised by _emscripten_throw_longjmp to unwind the guest stack."""


class EmscriptenSjLj:
    """Host-side implementation of emscripten-style setjmp/longjmp imports.

    native=True (default): the invoke_* trampolines and the longjmp thrower are
    implemented inside the Rust extension (no Python code runs per invoke).
    native=False: the same logic in Python, kept as the readable reference.
    """

    def __init__(self, native: bool = True):
        self.sb: Optional[Sandbox] = None
        self.native = native
        self._unwinds = 0
        self._invokes = 0

    @property
    def invokes(self) -> int:
        if self.native and self.sb is not None:
            return self.sb.store.sjlj_stats[0]
        return self._invokes

    @property
    def unwinds(self) -> int:
        if self.native and self.sb is not None:
            return self.sb.store.sjlj_stats[1]
        return self._unwinds

    @property
    def stack_guard_hits(self) -> int:
        if self.native and self.sb is not None:
            return self.sb.store.sjlj_stats[2]
        return 0

    def resolve(self, module: str, name: str, imp: dict):
        if module != "env":
            return None
        if name == "_emscripten_throw_longjmp":
            return NativeLongjmp() if self.native else self._throw_longjmp
        if name.startswith("invoke_"):
            return NativeInvoke(overflow_export="mp_sandbox_recursion_error") if self.native else self._make_invoke(bool(imp["results"]))
        return None

    def _throw_longjmp(self) -> None:
        self._unwinds += 1
        raise _LongjmpUnwind()

    def _make_invoke(self, has_result: bool):
        def invoke(index: int, *args):
            self._invokes += 1
            sb = self.sb
            sp = sb.call("stack_save", timeout=None)
            try:
                return sb.call_indirect(index, *args, timeout=None)
            except _LongjmpUnwind:
                sb.call("stack_restore", sp, timeout=None)
                sb.call("setThrew", 1, 0, timeout=None)
                return 0 if has_result else None

        return invoke


class MicroPython:
    """A MicroPython interpreter running inside a wasmi sandbox.

    functions: {"name": python_callable} callable from the guest as
    host.call("name", *args) (arguments/results travel as JSON).
    """

    def __init__(
        self,
        *,
        wasm_path=None,
        heap_size: int = 256 * 1024,
        max_memory: int = 64 * 1024 * 1024,
        fuel: Optional[int] = None,
        timeout: Optional[float] = None,
        functions: Optional[Dict[str, Callable[..., Any]]] = None,
        native_sjlj: bool = True,
    ):
        self.functions: Dict[str, Callable[..., Any]] = dict(functions or {})
        self._pending = b""
        self.output = bytearray()
        self.error_output = bytearray()
        self.sjlj = EmscriptenSjLj(native=native_sjlj)
        path = wasm_path or guest_path("micropython.wasm")

        def resolve(module, name, imp):
            if module == "env":
                own = {
                    "host_write": self._host_write,
                    "host_call": self._host_call,
                    "host_take": self._host_take,
                }.get(name)
                if own is not None:
                    return own
            return self.sjlj.resolve(module, name, imp)

        # Recursion is bounded by the host stack guard and MicroPython's own
        # shadow-stack check, so relax wasmi's per-store frame limits.
        engine = Engine(max_recursion_depth=200_000, max_stack_height=256 << 20)
        self.sb = Sandbox(path, engine=engine, imports=resolve, max_memory=max_memory, fuel=fuel, timeout=timeout)
        self.sjlj.sb = self.sb
        status = self.sb.call("mp_sandbox_init", heap_size)
        if status != 0:
            raise RuntimeError(f"mp_sandbox_init failed with status {status}")

    # -- host side ---------------------------------------------------------
    def register(self, name: str, fn: Callable[..., Any]) -> None:
        self.functions[name] = fn

    def function(self, fn: Callable[..., Any]) -> Callable[..., Any]:
        self.functions[fn.__name__] = fn
        return fn

    def _host_write(self, fd: int, ptr: int, length: int) -> None:
        data = self.sb.read(ptr, length)
        (self.output if fd == 1 else self.error_output).extend(data)

    def _host_call(self, name_ptr: int, name_len: int, args_ptr: int, args_len: int) -> int:
        name = self.sb.read(name_ptr, name_len).decode()
        try:
            args = json.loads(self.sb.read(args_ptr, args_len))
            fn = self.functions.get(name)
            if fn is None:
                raise NameError(f"no host function named {name!r}")
            payload = json.dumps(fn(*args)).encode()
            self._pending = payload
            return len(payload)
        except Exception as e:
            self._pending = f"{type(e).__name__}: {e}".encode()
            return -len(self._pending)

    def _host_take(self, ptr: int, length: int) -> None:
        self.sb.write(ptr, self._pending[:length])

    # -- guest side --------------------------------------------------------
    def exec(self, code: str, *, timeout=_NO_TIMEOUT, fuel: Optional[int] = None) -> str:
        """Compile and run Python source in the guest. Returns captured stdout.
        Raises PythonError for uncaught guest exceptions and Timeout /
        OutOfFuel / Trap for hard limit violations.

        fuel: if given, (re)sets the fuel budget before running this code."""
        if fuel is not None:
            self.sb.fuel = fuel
        self.error_output.clear()
        src = code.encode()
        ptr = self.sb.alloc(src, nul=True)
        try:
            status = self.sb.call("mp_sandbox_exec", ptr, len(src), timeout=timeout)
        finally:
            self.sb.free(ptr)
        if status != 0:
            raise PythonError(bytes(self.error_output).decode("utf-8", "replace"))
        return self.take_output()

    def take_output(self) -> str:
        out = bytes(self.output).decode("utf-8", "replace")
        self.output.clear()
        return out

    def collect(self) -> None:
        self.sb.call("mp_sandbox_collect")

    @property
    def memory_size(self) -> int:
        return self.sb.memory_size

    @property
    def fuel_consumed(self) -> int:
        return self.sb.fuel_consumed
