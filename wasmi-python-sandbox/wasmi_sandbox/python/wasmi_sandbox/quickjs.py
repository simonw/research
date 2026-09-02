"""Run untrusted JavaScript in QuickJS (quickjs-ng) compiled to WebAssembly."""

from __future__ import annotations

import json
import time
from typing import Any, Callable, Dict, Optional

from ._core import Engine
from .sandbox import Sandbox, guest_path, _NO_TIMEOUT



class JSError(Exception):
    """A JavaScript exception escaped from the guest."""


class QuickJS:
    """A QuickJS interpreter running inside a wasmi sandbox.

    max_memory: hard cap on the wasm linear memory (bytes).
    js_memory_limit: QuickJS's own soft allocation limit (bytes, 0 = none).
    fuel / timeout: wasmi-level CPU limits (raise OutOfFuel / Timeout).
    functions: {"name": python_callable} callable from JS as host.name(...).
    """

    def __init__(
        self,
        *,
        wasm_path=None,
        max_memory: int = 64 * 1024 * 1024,
        js_memory_limit: int = 0,
        stack_size: int = 512 * 1024,
        fuel: Optional[int] = None,
        timeout: Optional[float] = None,
        functions: Optional[Dict[str, Callable[..., Any]]] = None,
    ):
        self.functions: Dict[str, Callable[..., Any]] = dict(functions or {})
        self._pending = b""
        self._soft_deadline: Optional[float] = None
        self.output = bytearray()
        self.error_output = bytearray()
        self._result = bytearray()
        self._exception = bytearray()
        path = wasm_path or guest_path("quickjs.wasm")
        # Generous wasm-level stack limits so QuickJS's own (catchable) stack
        # overflow check - bounded by stack_size on the shadow stack - fires first.
        engine = Engine(max_recursion_depth=200_000, max_stack_height=256 << 20)
        self.sb = Sandbox(
            path,
            engine=engine,
            imports={
                "env": {
                    "host_write": self._host_write,
                    "host_call": self._host_call,
                    "host_take": self._host_take,
                    "host_interrupt": self._host_interrupt,
                }
            },
            max_memory=max_memory,
            fuel=fuel,
            timeout=timeout,
        )
        status = self.sb.call("qjs_init", js_memory_limit, stack_size)
        if status != 0:
            raise RuntimeError(f"qjs_init failed with status {status}")

    # -- host side ---------------------------------------------------------
    def register(self, name: str, fn: Callable[..., Any]) -> None:
        self.functions[name] = fn

    def function(self, fn: Callable[..., Any]) -> Callable[..., Any]:
        """Decorator: expose a Python function to JS as host.<name>(...)."""
        self.functions[fn.__name__] = fn
        return fn

    def _host_write(self, fd: int, ptr: int, length: int) -> None:
        data = self.sb.read(ptr, length)
        {1: self.output, 2: self.error_output, 3: self._result, 4: self._exception}[fd].extend(data)

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
        except Exception as e:  # report to the guest as a JS exception
            self._pending = f"{type(e).__name__}: {e}".encode()
            return -len(self._pending)

    def _host_take(self, ptr: int, length: int) -> None:
        self.sb.write(ptr, self._pending[:length])

    def _host_interrupt(self) -> int:
        if self._soft_deadline is not None and time.monotonic() > self._soft_deadline:
            return 1
        return 0

    # -- guest side --------------------------------------------------------
    def eval(self, code: str, *, timeout=_NO_TIMEOUT, fuel: Optional[int] = None, soft_timeout: Optional[float] = None) -> Any:
        """Evaluate JS source. Returns the completion value (decoded from JSON
        when possible). Raises JSError for JS exceptions, and wasmi_sandbox's
        Timeout / OutOfFuel / Trap for hard limit violations.

        fuel: if given, (re)sets the fuel budget before running this code.
        soft_timeout: cooperative limit via QuickJS's interrupt handler; the
        guest sees an InternalError and stays consistent."""
        if fuel is not None:
            self.sb.fuel = fuel
        self._result.clear()
        self._exception.clear()
        self._soft_deadline = time.monotonic() + soft_timeout if soft_timeout else None
        src = code.encode()
        ptr = self.sb.alloc(src, nul=True)
        try:
            status = self.sb.call("qjs_eval", ptr, len(src), timeout=timeout)
        finally:
            self.sb.free(ptr)
            self._soft_deadline = None
        if status != 0:
            raise JSError(bytes(self._exception).decode("utf-8", "replace"))
        text = bytes(self._result).decode("utf-8", "replace")
        try:
            return json.loads(text)
        except ValueError:
            return text

    def take_output(self) -> str:
        out = bytes(self.output).decode("utf-8", "replace")
        self.output.clear()
        return out

    def gc(self) -> None:
        self.sb.call("qjs_gc")

    @property
    def js_memory_usage(self) -> int:
        return self.sb.call("qjs_memory_usage")

    @property
    def memory_size(self) -> int:
        return self.sb.memory_size

    @property
    def fuel_consumed(self) -> int:
        return self.sb.fuel_consumed
