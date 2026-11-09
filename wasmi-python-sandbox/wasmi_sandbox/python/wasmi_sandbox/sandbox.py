"""High-level sandbox wrapper: module loading, import resolution, a tiny WASI
implementation written in Python, and memory helpers."""

from __future__ import annotations

import os
import struct
import time
from pathlib import Path
from typing import Any, Callable, Dict, Mapping, Optional, Union

from ._core import Engine, Exit, LinkError, Module, Store

ImportResolver = Callable[[str, str, dict], Optional[Callable[..., Any]]]
Imports = Union[Mapping[str, Mapping[str, Callable[..., Any]]], ImportResolver]

_NO_TIMEOUT = object()


class NativeInvoke:
    """Resolver result: implement this emscripten-style invoke_* import in Rust."""

    def __init__(self, table: str = "__indirect_function_table", stack_pointer_global: str = "__stack_pointer", overflow_export: Optional[str] = None):
        self.table = table
        self.stack_pointer_global = stack_pointer_global
        # Guest export to call instead when the host's native stack runs low
        # (it should raise a guest-level exception via longjmp).
        self.overflow_export = overflow_export


class NativeLongjmp:
    """Resolver result: implement _emscripten_throw_longjmp in Rust."""


class WasiLite:
    """A deliberately tiny WASI preview1 implementation.

    Provides stdio (captured in memory), clocks, randomness, args/environ and
    nothing else: there is no filesystem, no network and no preopened
    directories. Every other WASI call answers ENOSYS or ENOTCAPABLE.
    """

    ESUCCESS = 0
    EBADF = 8
    EINVAL = 28
    ENOENT = 44
    ENOSYS = 52
    ENOTSUP = 58
    ESPIPE = 70
    ENOTCAPABLE = 76

    def __init__(self, sandbox: "Sandbox", stdin: bytes = b"", args=("guest",), env: Optional[Mapping[str, str]] = None):
        self.sb = sandbox
        self.stdout = bytearray()
        self.stderr = bytearray()
        self.stdin = bytes(stdin)
        self.stdin_pos = 0
        self.args = [str(a).encode() + b"\0" for a in args]
        self.env = [f"{k}={v}".encode() + b"\0" for k, v in (env or {}).items()]
        self.sinks: Dict[int, Callable[[bytes], None]] = {}

    # -- helpers -----------------------------------------------------------
    def lookup(self, name: str, imp: dict) -> Callable[..., Any]:
        fn = getattr(self, name, None)
        if fn is not None and callable(fn):
            return fn
        n_results = len(imp["results"])

        def stub(*_args, _name=name):
            self.sb.unsupported_wasi_calls.append(_name)
            return self.ENOSYS if n_results else None

        return stub

    def _write_iovs(self, fd: int, iovs: int, iovs_len: int) -> int:
        total = 0
        chunks = []
        for i in range(iovs_len):
            ptr, length = struct.unpack("<II", self.sb.read(iovs + 8 * i, 8))
            chunks.append(self.sb.read(ptr, length))
            total += length
        data = b"".join(chunks)
        sink = self.sinks.get(fd)
        if sink is not None:
            sink(data)
        elif fd == 1:
            self.stdout += data
        elif fd == 2:
            self.stderr += data
        return total

    # -- WASI functions ----------------------------------------------------
    def args_sizes_get(self, argc_ptr, buf_size_ptr):
        self.sb.write_u32(argc_ptr, len(self.args))
        self.sb.write_u32(buf_size_ptr, sum(len(a) for a in self.args))
        return self.ESUCCESS

    def args_get(self, argv_ptr, buf_ptr):
        return self._strings_get(self.args, argv_ptr, buf_ptr)

    def environ_sizes_get(self, count_ptr, buf_size_ptr):
        self.sb.write_u32(count_ptr, len(self.env))
        self.sb.write_u32(buf_size_ptr, sum(len(e) for e in self.env))
        return self.ESUCCESS

    def environ_get(self, environ_ptr, buf_ptr):
        return self._strings_get(self.env, environ_ptr, buf_ptr)

    def _strings_get(self, items, ptrs, buf):
        for i, item in enumerate(items):
            self.sb.write_u32(ptrs + 4 * i, buf)
            self.sb.write(buf, item)
            buf += len(item)
        return self.ESUCCESS

    def clock_res_get(self, clock_id, res_ptr):
        self.sb.write_u64(res_ptr, 1000)
        return self.ESUCCESS

    def clock_time_get(self, clock_id, precision, time_ptr):
        if clock_id == 0:
            now = time.time_ns()
        else:
            now = time.monotonic_ns()
        self.sb.write_u64(time_ptr, now)
        return self.ESUCCESS

    def fd_write(self, fd, iovs, iovs_len, nwritten_ptr):
        if fd not in (1, 2) and fd not in self.sinks:
            return self.EBADF
        n = self._write_iovs(fd, iovs, iovs_len)
        self.sb.write_u32(nwritten_ptr, n)
        return self.ESUCCESS

    def fd_read(self, fd, iovs, iovs_len, nread_ptr):
        if fd != 0:
            return self.EBADF
        total = 0
        for i in range(iovs_len):
            ptr, length = struct.unpack("<II", self.sb.read(iovs + 8 * i, 8))
            chunk = self.stdin[self.stdin_pos : self.stdin_pos + length]
            if not chunk:
                break
            self.sb.write(ptr, chunk)
            self.stdin_pos += len(chunk)
            total += len(chunk)
        self.sb.write_u32(nread_ptr, total)
        return self.ESUCCESS

    def fd_close(self, fd):
        return self.ESUCCESS if fd in (0, 1, 2) else self.EBADF

    def fd_fdstat_get(self, fd, buf):
        if fd not in (0, 1, 2):
            return self.EBADF
        # filetype=character device (2), flags=0, rights=all, inheriting=all
        self.sb.write(buf, struct.pack("<BxHxxxxQQ", 2, 0, 0xFFFFFFFFFFFFFFFF, 0xFFFFFFFFFFFFFFFF))
        return self.ESUCCESS

    def fd_fdstat_set_flags(self, fd, flags):
        return self.ESUCCESS if fd in (0, 1, 2) else self.EBADF

    def fd_seek(self, fd, offset, whence, newoffset_ptr):
        return self.ESPIPE if fd in (0, 1, 2) else self.EBADF

    def fd_prestat_get(self, fd, buf):
        return self.EBADF  # no preopened directories

    def fd_prestat_dir_name(self, fd, path, path_len):
        return self.EBADF

    def fd_filestat_get(self, fd, buf):
        return self.EBADF

    def fd_sync(self, fd):
        return self.ESUCCESS

    def path_open(self, *args):
        return self.ENOTCAPABLE

    def path_filestat_get(self, *args):
        return self.ENOTCAPABLE

    def path_unlink_file(self, *args):
        return self.ENOTCAPABLE

    def path_create_directory(self, *args):
        return self.ENOTCAPABLE

    def poll_oneoff(self, *args):
        return self.ENOSYS

    def proc_exit(self, code):
        raise Exit(code)

    def random_get(self, buf, length):
        self.sb.write(buf, os.urandom(length))
        return self.ESUCCESS

    def sched_yield(self):
        return self.ESUCCESS


class Sandbox:
    """A WebAssembly module instance with resource limits.

    max_memory: cap in bytes on the guest's linear memory.
    fuel: fuel budget shared by all calls (None = unlimited).
    timeout: default wall-clock limit in seconds per call (needs fuel metering).
    imports: {"module": {"name": callable}} or a resolver(module, name, info).
    wasi: provide the built-in WASI-lite implementation for wasi imports.
    """

    def __init__(
        self,
        wasm: Union[bytes, str, Path],
        *,
        engine: Optional[Engine] = None,
        imports: Optional[Imports] = None,
        max_memory: Optional[int] = None,
        fuel: Optional[int] = None,
        timeout: Optional[float] = None,
        fuel_slice: int = 250_000,
        trap_on_grow_failure: bool = False,
        wasi: bool = True,
        stdin: bytes = b"",
        args=("guest",),
        env: Optional[Mapping[str, str]] = None,
        initialize: bool = True,
    ):
        if not isinstance(wasm, (bytes, bytearray)):
            wasm = Path(wasm).read_bytes()
        self.engine = engine or Engine()
        self.module = Module(self.engine, bytes(wasm))
        self.store = Store(
            self.engine,
            max_memory_bytes=max_memory,
            fuel=fuel,
            fuel_slice=fuel_slice,
            trap_on_grow_failure=trap_on_grow_failure,
        )
        self.timeout = timeout
        self.unsupported_wasi_calls: list = []
        self.wasi = WasiLite(self, stdin=stdin, args=args, env=env) if wasi else None

        if imports is None:
            resolver: ImportResolver = lambda m, n, i: None
        elif callable(imports):
            resolver = imports
        else:
            resolver = lambda m, n, i, _d=imports: _d.get(m, {}).get(n)

        missing = []
        for imp in self.module.imports():
            if imp["kind"] != "func":
                raise LinkError(f"unsupported non-function import {imp['module']}.{imp['name']} ({imp['kind']})")
            fn = resolver(imp["module"], imp["name"], imp)
            if fn is None and self.wasi is not None and imp["module"] in ("wasi_snapshot_preview1", "wasi_unstable"):
                fn = self.wasi.lookup(imp["name"], imp)
            if fn is None:
                missing.append(f"{imp['module']}.{imp['name']}")
                continue
            if isinstance(fn, NativeInvoke):
                self.store.define_invoke(imp["module"], imp["name"], imp["params"], imp["results"], fn.table, fn.stack_pointer_global, fn.overflow_export)
            elif isinstance(fn, NativeLongjmp):
                self.store.define_longjmp_thrower(imp["module"], imp["name"])
            else:
                self.store.define_func(imp["module"], imp["name"], imp["params"], imp["results"], fn)
        if missing:
            raise LinkError("unresolved imports: " + ", ".join(missing))

        self.store.instantiate(self.module)
        self.exports = {name: kind for name, kind in self.store.exports()}
        if initialize and self.exports.get("_initialize") == "func":
            self.call("_initialize")

    # -- calls -------------------------------------------------------------
    def call(self, name: str, *args, timeout=_NO_TIMEOUT):
        t = self.timeout if timeout is _NO_TIMEOUT else timeout
        return self.store.call(name, args, timeout=t)

    def call_indirect(self, index: int, *args, table: str = "__indirect_function_table", timeout=_NO_TIMEOUT):
        t = self.timeout if timeout is _NO_TIMEOUT else timeout
        return self.store.call_indirect(table, index, args, timeout=t)

    # -- fuel --------------------------------------------------------------
    @property
    def fuel(self) -> Optional[int]:
        return self.store.fuel

    @fuel.setter
    def fuel(self, value: Optional[int]) -> None:
        self.store.fuel = value

    @property
    def fuel_consumed(self) -> int:
        return self.store.fuel_consumed

    # -- memory ------------------------------------------------------------
    @property
    def memory_size(self) -> int:
        return self.store.memory_size()

    def read(self, ptr: int, length: int) -> bytes:
        return self.store.memory_read(ptr, length)

    def write(self, ptr: int, data: bytes) -> None:
        self.store.memory_write(ptr, bytes(data))

    def read_cstr(self, ptr: int, max_len: int = 1 << 20) -> bytes:
        return self.store.memory_read_cstr(ptr, max_len)

    def read_u32(self, ptr: int) -> int:
        return struct.unpack("<I", self.read(ptr, 4))[0]

    def write_u32(self, ptr: int, value: int) -> None:
        self.write(ptr, struct.pack("<I", value & 0xFFFFFFFF))

    def write_u64(self, ptr: int, value: int) -> None:
        self.write(ptr, struct.pack("<Q", value & 0xFFFFFFFFFFFFFFFF))

    def alloc(self, data: bytes, nul: bool = False) -> int:
        """Copy data into guest memory using the guest's exported malloc."""
        payload = bytes(data) + (b"\0" if nul else b"")
        ptr = self.call("malloc", len(payload), timeout=None)
        if ptr == 0:
            raise MemoryError("guest malloc failed")
        self.write(ptr, payload)
        return ptr

    def free(self, ptr: int) -> None:
        self.call("free", ptr, timeout=None)

    # -- stdio -------------------------------------------------------------
    @property
    def stdout(self) -> str:
        return bytes(self.wasi.stdout).decode("utf-8", "replace") if self.wasi else ""

    @property
    def stderr(self) -> str:
        return bytes(self.wasi.stderr).decode("utf-8", "replace") if self.wasi else ""


def guest_path(name: str) -> Path:
    """Locate a prebuilt guest .wasm (package data first, then the repo checkout)."""
    env = os.environ.get("WASMI_SANDBOX_GUESTS")
    candidates = []
    if env:
        candidates.append(Path(env) / name)
    here = Path(__file__).resolve()
    candidates.append(here.parent / "guests" / name)
    stem = name.rsplit(".", 1)[0]
    candidates.append(here.parents[2] / "guests" / stem / name)
    for c in candidates:
        if c.exists():
            return c
    raise FileNotFoundError(f"could not find {name}; looked in {[str(c) for c in candidates]}")
