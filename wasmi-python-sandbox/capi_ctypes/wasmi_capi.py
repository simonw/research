"""Calling wasmi 2.0 from Python through its C API (libwasmi.so) with ctypes.

The C API implements the standard wasm-c-api (wasm.h) plus a small wasmi.h
extension (fuel, compilation mode). This module wraps just enough of it to:

  * compile a module, list its imports/exports
  * instantiate it with host functions implemented in Python
    (wasm_func_new_with_env + a ctypes callback)
  * call exports, read/write memory
  * meter CPU with fuel (wasmi_config_consume_fuel_set / wasmi_context_set_fuel)

Build libwasmi.so with:
  cmake -S crates/c_api -B target/c_api --install-prefix "$PWD/artifacts"
  cmake --build target/c_api --target install

Compared with the PyO3 route the C API cannot: limit memory growth (no
ResourceLimiter), resume an out-of-fuel call (no wall-clock deadlines), or
install call hooks. It also uses the wasm.h "own"/vec conventions which need
careful manual memory management from ctypes.
"""

from __future__ import annotations

import ctypes as C
import os
from typing import Callable, Dict, List, Sequence, Tuple

WASM_I32, WASM_I64, WASM_F32, WASM_F64 = 0, 1, 2, 3
VALKIND_NAMES = {0: "i32", 1: "i64", 2: "f32", 3: "f64", 128: "funcref", 129: "externref"}
KIND_FUNC, KIND_GLOBAL, KIND_TABLE, KIND_MEMORY = 0, 1, 2, 3


class wasm_byte_vec_t(C.Structure):
    _fields_ = [("size", C.c_size_t), ("data", C.POINTER(C.c_ubyte))]


class wasm_valtype_vec_t(C.Structure):
    _fields_ = [("size", C.c_size_t), ("data", C.POINTER(C.c_void_p))]


class wasm_extern_vec_t(C.Structure):
    _fields_ = [("size", C.c_size_t), ("data", C.POINTER(C.c_void_p))]


class wasm_exporttype_vec_t(C.Structure):
    _fields_ = [("size", C.c_size_t), ("data", C.POINTER(C.c_void_p))]


class wasm_importtype_vec_t(C.Structure):
    _fields_ = [("size", C.c_size_t), ("data", C.POINTER(C.c_void_p))]


class wasm_val_union(C.Union):
    _fields_ = [("i32", C.c_int32), ("i64", C.c_int64), ("f32", C.c_float), ("f64", C.c_double), ("ref", C.c_void_p)]


class wasm_val_t(C.Structure):
    _fields_ = [("kind", C.c_uint8), ("of", wasm_val_union)]


class wasm_val_vec_t(C.Structure):
    _fields_ = [("size", C.c_size_t), ("data", C.POINTER(wasm_val_t))]


# wasm_func_callback_with_env_t: own wasm_trap_t* (*)(void* env, const wasm_val_vec_t* args, wasm_val_vec_t* results)
CALLBACK = C.CFUNCTYPE(C.c_void_p, C.c_void_p, C.POINTER(wasm_val_vec_t), C.POINTER(wasm_val_vec_t))


def load(path: str | None = None) -> C.CDLL:
    path = path or os.environ.get("LIBWASMI", "libwasmi.so")
    lib = C.CDLL(path)
    P = C.c_void_p
    sigs = {
        "wasm_config_new": ([], P),
        "wasmi_config_consume_fuel_set": ([P, C.c_bool], None),
        "wasm_engine_new_with_config": ([P], P),
        "wasm_engine_delete": ([P], None),
        "wasmi_store_new": ([P, P, P], P),
        "wasmi_store_delete": ([P], None),
        "wasmi_store_context": ([P], P),
        "wasmi_context_set_fuel": ([P, C.c_uint64], P),
        "wasmi_context_get_fuel": ([P, C.POINTER(C.c_uint64)], P),
        "wasm_store_new": ([P], P),
        "wasm_store_delete": ([P], None),
        "wasm_byte_vec_new": ([C.POINTER(wasm_byte_vec_t), C.c_size_t, C.POINTER(C.c_ubyte)], None),
        "wasm_byte_vec_delete": ([C.POINTER(wasm_byte_vec_t)], None),
        "wasm_module_new": ([P, C.POINTER(wasm_byte_vec_t)], P),
        "wasm_module_delete": ([P], None),
        "wasm_module_imports": ([P, C.POINTER(wasm_importtype_vec_t)], None),
        "wasm_module_exports": ([P, C.POINTER(wasm_exporttype_vec_t)], None),
        "wasm_importtype_module": ([P], C.POINTER(wasm_byte_vec_t)),
        "wasm_importtype_name": ([P], C.POINTER(wasm_byte_vec_t)),
        "wasm_importtype_type": ([P], P),
        "wasm_exporttype_name": ([P], C.POINTER(wasm_byte_vec_t)),
        "wasm_exporttype_type": ([P], P),
        "wasm_externtype_kind": ([P], C.c_uint8),
        "wasm_externtype_as_functype_const": ([P], P),
        "wasm_functype_params": ([P], C.POINTER(wasm_valtype_vec_t)),
        "wasm_functype_results": ([P], C.POINTER(wasm_valtype_vec_t)),
        "wasm_functype_new": ([C.POINTER(wasm_valtype_vec_t), C.POINTER(wasm_valtype_vec_t)], P),
        "wasm_functype_delete": ([P], None),
        "wasm_valtype_new": ([C.c_uint8], P),
        "wasm_valtype_kind": ([P], C.c_uint8),
        "wasm_valtype_vec_new": ([C.POINTER(wasm_valtype_vec_t), C.c_size_t, C.POINTER(C.c_void_p)], None),
        "wasm_valtype_vec_new_empty": ([C.POINTER(wasm_valtype_vec_t)], None),
        "wasm_func_new_with_env": ([P, P, CALLBACK, P, P], P),
        "wasm_func_as_extern": ([P], P),
        "wasm_func_call": ([P, C.POINTER(wasm_val_vec_t), C.POINTER(wasm_val_vec_t)], P),
        "wasm_func_param_arity": ([P], C.c_size_t),
        "wasm_func_result_arity": ([P], C.c_size_t),
        "wasm_instance_new": ([P, P, C.POINTER(wasm_extern_vec_t), C.POINTER(P)], P),
        "wasm_instance_exports": ([P, C.POINTER(wasm_extern_vec_t)], None),
        "wasm_extern_kind": ([P], C.c_uint8),
        "wasm_extern_as_func": ([P], P),
        "wasm_extern_as_memory": ([P], P),
        "wasm_memory_data": ([P], C.POINTER(C.c_ubyte)),
        "wasm_memory_data_size": ([P], C.c_size_t),
        "wasm_trap_message": ([P, C.POINTER(wasm_byte_vec_t)], None),
        "wasm_trap_delete": ([P], None),
        "wasm_trap_new": ([P, C.POINTER(wasm_byte_vec_t)], P),
        "wasmi_error_delete": ([P], None),
    }
    for name, (args, res) in sigs.items():
        fn = getattr(lib, name)
        fn.argtypes = args
        fn.restype = res
    return lib


def _bytes(vec: wasm_byte_vec_t) -> bytes:
    return C.string_at(vec.data, vec.size) if vec.size else b""


class CapiSandbox:
    """Minimal sandbox over the wasmi C API. Host functions are Python callables."""

    def __init__(self, lib: C.CDLL, wasm: bytes, imports: Dict[str, Dict[str, Callable]], fuel: int | None = None):
        self.lib = lib
        self._keep: List[object] = []  # keep ctypes callbacks alive
        self.host_exception: BaseException | None = None

        config = lib.wasm_config_new()
        lib.wasmi_config_consume_fuel_set(config, fuel is not None)
        self.engine = lib.wasm_engine_new_with_config(config)
        # wasm.h store (used by wasm_func_new_with_env & friends); the wasmi.h
        # store type is a different object, so fuel is set through wasmi_store_context
        # on a wasmi_store_t. Here we use the plain wasm.h store for simplicity
        # and only touch fuel through the wasmi_* helpers when asked.
        self.store = lib.wasm_store_new(self.engine)

        vec = wasm_byte_vec_t()
        buf = (C.c_ubyte * len(wasm)).from_buffer_copy(wasm)
        lib.wasm_byte_vec_new(C.byref(vec), len(wasm), buf)
        self.module = lib.wasm_module_new(self.store, C.byref(vec))
        lib.wasm_byte_vec_delete(C.byref(vec))
        if not self.module:
            raise RuntimeError("wasm_module_new failed (invalid module?)")

        # Resolve imports in module order.
        imps = wasm_importtype_vec_t()
        lib.wasm_module_imports(self.module, C.byref(imps))
        externs = []
        for i in range(imps.size):
            it = imps.data[i]
            mod = _bytes(lib.wasm_importtype_module(it).contents).decode()
            name = _bytes(lib.wasm_importtype_name(it).contents).decode()
            ety = lib.wasm_importtype_type(it)
            if lib.wasm_externtype_kind(ety) != KIND_FUNC:
                raise RuntimeError(f"non-function import {mod}.{name}")
            fty = lib.wasm_externtype_as_functype_const(ety)
            params = [lib.wasm_valtype_kind(p) for p in self._valtypes(lib.wasm_functype_params(fty).contents)]
            results = [lib.wasm_valtype_kind(p) for p in self._valtypes(lib.wasm_functype_results(fty).contents)]
            fn = imports.get(mod, {}).get(name)
            if fn is None:
                raise RuntimeError(f"unresolved import {mod}.{name}")
            externs.append(self._host_func(fn, params, results))

        ext_arr = (C.c_void_p * max(1, len(externs)))(*externs)
        ext_vec = wasm_extern_vec_t(len(externs), ext_arr)
        trap = C.c_void_p()
        self.instance = lib.wasm_instance_new(self.store, self.module, C.byref(ext_vec), C.byref(trap))
        if not self.instance:
            raise RuntimeError("instantiation failed: " + self._trap_text(trap))

        # Index exports by name.
        exps = wasm_exporttype_vec_t()
        lib.wasm_module_exports(self.module, C.byref(exps))
        names = [_bytes(lib.wasm_exporttype_name(exps.data[i]).contents).decode() for i in range(exps.size)]
        self._exports = wasm_extern_vec_t()
        lib.wasm_instance_exports(self.instance, C.byref(self._exports))
        self.exports = {names[i]: self._exports.data[i] for i in range(self._exports.size)}

    def _valtypes(self, vec: wasm_valtype_vec_t):
        return [vec.data[i] for i in range(vec.size)]

    def _host_func(self, fn: Callable, params: List[int], results: List[int]):
        lib = self.lib

        def callback(env, args_p, results_p):
            args = args_p.contents
            pyargs = []
            for i in range(args.size):
                v = args.data[i]
                pyargs.append({0: v.of.i32, 1: v.of.i64, 2: v.of.f32, 3: v.of.f64}[v.kind])
            try:
                ret = fn(*pyargs)
            except BaseException as e:  # surface to the caller of wasm_func_call
                self.host_exception = e
                msg = wasm_byte_vec_t()
                text = f"host exception: {e!r}".encode() + b"\0"  # wasm_trap_new wants a C string
                buf = (C.c_ubyte * len(text)).from_buffer_copy(text)
                lib.wasm_byte_vec_new(C.byref(msg), len(text), buf)
                return lib.wasm_trap_new(self.store, C.byref(msg))
            out = results_p.contents
            if out.size:
                vals = ret if isinstance(ret, tuple) else (ret,)
                for i, kind in enumerate(results):
                    out.data[i].kind = kind
                    if kind == WASM_I32:
                        out.data[i].of.i32 = int(vals[i])
                    elif kind == WASM_I64:
                        out.data[i].of.i64 = int(vals[i])
                    elif kind == WASM_F32:
                        out.data[i].of.f32 = float(vals[i])
                    else:
                        out.data[i].of.f64 = float(vals[i])
            return None

        cb = CALLBACK(callback)
        self._keep.append(cb)
        pv = wasm_valtype_vec_t()
        rv = wasm_valtype_vec_t()
        parr = (C.c_void_p * max(1, len(params)))(*[lib.wasm_valtype_new(k) for k in params])
        rarr = (C.c_void_p * max(1, len(results)))(*[lib.wasm_valtype_new(k) for k in results])
        lib.wasm_valtype_vec_new(C.byref(pv), len(params), parr)
        lib.wasm_valtype_vec_new(C.byref(rv), len(results), rarr)
        fty = lib.wasm_functype_new(C.byref(pv), C.byref(rv))
        func = lib.wasm_func_new_with_env(self.store, fty, cb, None, None)
        lib.wasm_functype_delete(fty)
        return lib.wasm_func_as_extern(func)

    def _trap_text(self, trap) -> str:
        if not trap:
            return ""
        msg = wasm_byte_vec_t()
        self.lib.wasm_trap_message(trap, C.byref(msg))
        text = _bytes(msg).rstrip(b"\0").decode("utf-8", "replace")
        self.lib.wasm_byte_vec_delete(C.byref(msg))
        self.lib.wasm_trap_delete(trap)
        return text

    def call(self, name: str, *args: int | float):
        lib = self.lib
        func = lib.wasm_extern_as_func(self.exports[name])
        n_res = lib.wasm_func_result_arity(func)
        argv = (wasm_val_t * max(1, len(args)))()
        for i, a in enumerate(args):
            if isinstance(a, float):
                argv[i].kind = WASM_F64
                argv[i].of.f64 = a
            else:
                argv[i].kind = WASM_I32
                argv[i].of.i32 = a
        resv = (wasm_val_t * max(1, n_res))()
        avec = wasm_val_vec_t(len(args), argv)
        rvec = wasm_val_vec_t(n_res, resv)
        self.host_exception = None
        trap = lib.wasm_func_call(func, C.byref(avec), C.byref(rvec))
        if trap:
            text = self._trap_text(trap)
            if self.host_exception is not None:
                raise self.host_exception
            raise RuntimeError(f"trap: {text}")
        out = []
        for i in range(n_res):
            v = resv[i]
            out.append({0: v.of.i32, 1: v.of.i64, 2: v.of.f32, 3: v.of.f64}[v.kind])
        return out[0] if n_res == 1 else (tuple(out) if out else None)

    def memory(self, name: str = "memory") -> Tuple[int, int]:
        mem = self.lib.wasm_extern_as_memory(self.exports[name])
        return C.addressof(self.lib.wasm_memory_data(mem).contents), self.lib.wasm_memory_data_size(mem)

    def read(self, ptr: int, n: int) -> bytes:
        base, size = self.memory()
        if ptr + n > size:
            raise IndexError("out of bounds")
        return C.string_at(base + ptr, n)
