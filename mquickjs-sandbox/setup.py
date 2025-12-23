#!/usr/bin/env python3
"""
Setup script to build the mquickjs Python C extension.

Usage:
    python setup.py build_ext --inplace
"""

import os
import subprocess
import shutil
from pathlib import Path
from setuptools import setup, Extension

MQUICKJS_DIR = Path("/tmp/mquickjs")
BUILD_DIR = Path("/tmp/mquickjs-ext-build")


def ensure_mquickjs():
    """Ensure mquickjs is cloned and headers are generated."""
    if not MQUICKJS_DIR.exists():
        print("Cloning mquickjs...")
        subprocess.run(["git", "clone", "https://github.com/bellard/mquickjs.git", str(MQUICKJS_DIR)],
                       check=True)

    # Create build directory
    BUILD_DIR.mkdir(exist_ok=True)

    # Copy source files
    for f in ["mquickjs.c", "mquickjs.h", "mquickjs_priv.h", "mquickjs_opcode.h",
              "cutils.c", "cutils.h", "dtoa.c", "dtoa.h", "libm.c", "libm.h",
              "list.h", "mquickjs_build.c", "mquickjs_build.h", "mqjs_stdlib.c",
              "softfp_template.h", "softfp_template_icvt.h"]:
        src = MQUICKJS_DIR / f
        if src.exists():
            shutil.copy(src, BUILD_DIR / f)

    # Check if headers need to be generated
    if not (BUILD_DIR / "mqjs_stdlib.h").exists():
        print("Generating stdlib headers...")
        subprocess.run(["gcc", "-Wall", "-g", "-O2", "-D_GNU_SOURCE", "-fno-math-errno",
                        "-fno-trapping-math", "-c", "-o", "mqjs_stdlib.host.o", "mqjs_stdlib.c"],
                       cwd=BUILD_DIR, check=True)
        subprocess.run(["gcc", "-Wall", "-g", "-O2", "-D_GNU_SOURCE", "-fno-math-errno",
                        "-fno-trapping-math", "-c", "-o", "mquickjs_build.host.o", "mquickjs_build.c"],
                       cwd=BUILD_DIR, check=True)
        subprocess.run(["gcc", "-g", "-o", "mqjs_stdlib_gen", "mqjs_stdlib.host.o", "mquickjs_build.host.o"],
                       cwd=BUILD_DIR, check=True)

        result = subprocess.run(["./mqjs_stdlib_gen"], cwd=BUILD_DIR, capture_output=True, text=True, check=True)
        (BUILD_DIR / "mqjs_stdlib.h").write_text(result.stdout)

        result = subprocess.run(["./mqjs_stdlib_gen", "-a"], cwd=BUILD_DIR, capture_output=True, text=True, check=True)
        (BUILD_DIR / "mquickjs_atom.h").write_text(result.stdout)


# Prepare the build
ensure_mquickjs()

# Create the extension module source with embedded functions
ext_source = Path(__file__).parent / "mquickjs_ext_combined.c"
with open(ext_source, "w") as f:
    # Write the combined source
    f.write('''
/* Combined mquickjs extension source */
#define PY_SSIZE_T_CLEAN
#include <Python.h>

/* Define required functions before including mqjs_stdlib.h */
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <stdint.h>
#include <stdio.h>

#include "cutils.h"
#include "mquickjs.h"

/* Forward declarations for stdlib */
static JSValue js_print(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv);
static JSValue js_gc(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv);
static JSValue js_date_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv);
static JSValue js_performance_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv);
static JSValue js_load(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv);
static JSValue js_setTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv);
static JSValue js_clearTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv);

#include "mqjs_stdlib.h"

/* Implementations */
static JSValue js_print(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_UNDEFINED; }
static JSValue js_gc(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { JS_GC(ctx); return JS_UNDEFINED; }
static JSValue js_date_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_NewInt64(ctx, 0); }
static JSValue js_performance_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_NewInt64(ctx, 0); }
static JSValue js_load(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_ThrowTypeError(ctx, "load() disabled"); }
static JSValue js_setTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_ThrowTypeError(ctx, "setTimeout() disabled"); }
static JSValue js_clearTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_ThrowTypeError(ctx, "clearTimeout() disabled"); }

/* Sandbox context */
typedef struct {
    PyObject_HEAD
    JSContext *ctx;
    uint8_t *mem_buf;
    size_t mem_size;
    int64_t time_limit_ms;
    int64_t start_time_ms;
    int timed_out;
} SandboxObject;

static int64_t get_time_ms(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000 + (ts.tv_nsec / 1000000);
}

static int sandbox_interrupt_handler(JSContext *ctx, void *opaque)
{
    SandboxObject *self = (SandboxObject *)opaque;
    if (self->time_limit_ms > 0) {
        int64_t elapsed = get_time_ms() - self->start_time_ms;
        if (elapsed >= self->time_limit_ms) {
            self->timed_out = 1;
            return 1;
        }
    }
    return 0;
}

static PyObject *SandboxError;
static PyObject *TimeoutError_ext;
static PyObject *MemoryError_ext;

static void Sandbox_dealloc(SandboxObject *self)
{
    if (self->ctx) JS_FreeContext(self->ctx);
    if (self->mem_buf) free(self->mem_buf);
    Py_TYPE(self)->tp_free((PyObject *)self);
}

static int Sandbox_init(SandboxObject *self, PyObject *args, PyObject *kwds)
{
    static char *kwlist[] = {"memory_limit_bytes", "time_limit_ms", NULL};
    Py_ssize_t memory_limit = 1024 * 1024;
    Py_ssize_t time_limit = 1000;

    if (!PyArg_ParseTupleAndKeywords(args, kwds, "|nn", kwlist, &memory_limit, &time_limit))
        return -1;

    if (memory_limit < 8192) {
        PyErr_SetString(PyExc_ValueError, "memory_limit_bytes must be at least 8192");
        return -1;
    }

    self->mem_size = memory_limit;
    self->time_limit_ms = time_limit;
    self->timed_out = 0;
    self->mem_buf = malloc(memory_limit);
    if (!self->mem_buf) { PyErr_NoMemory(); return -1; }

    self->ctx = JS_NewContext(self->mem_buf, memory_limit, &js_stdlib);
    if (!self->ctx) { free(self->mem_buf); self->mem_buf = NULL; PyErr_SetString(MemoryError_ext, "Failed to create context"); return -1; }

    JS_SetContextOpaque(self->ctx, self);
    JS_SetInterruptHandler(self->ctx, sandbox_interrupt_handler);
    JS_SetRandomSeed(self->ctx, 12345);
    return 0;
}

static PyObject *convert_jsvalue_to_python(JSContext *ctx, JSValue val)
{
    if (JS_IsUndefined(val) || JS_IsNull(val)) Py_RETURN_NONE;
    if (JS_IsBool(val)) { if (val == JS_TRUE) Py_RETURN_TRUE; else Py_RETURN_FALSE; }
    if (JS_IsInt(val)) return PyLong_FromLong(JS_VALUE_GET_INT(val));
    if (JS_IsNumber(ctx, val)) { double d; JS_ToNumber(ctx, &d, val); return PyFloat_FromDouble(d); }
    if (JS_IsString(ctx, val)) {
        JSCStringBuf buf; size_t len;
        const char *str = JS_ToCStringLen(ctx, &len, val, &buf);
        if (str) return PyUnicode_FromStringAndSize(str, len);
        Py_RETURN_NONE;
    }
    JSValue str_val = JS_ToString(ctx, val);
    if (JS_IsString(ctx, str_val)) {
        JSCStringBuf buf; size_t len;
        const char *str = JS_ToCStringLen(ctx, &len, str_val, &buf);
        if (str) return PyUnicode_FromStringAndSize(str, len);
    }
    return PyUnicode_FromString("[object]");
}

static PyObject *Sandbox_execute(SandboxObject *self, PyObject *args)
{
    const char *code; Py_ssize_t code_len; JSValue val; char error_buf[4096];
    if (!PyArg_ParseTuple(args, "s#", &code, &code_len)) return NULL;
    if (!self->ctx) { PyErr_SetString(SandboxError, "Context closed"); return NULL; }

    self->start_time_ms = get_time_ms();
    self->timed_out = 0;
    Py_BEGIN_ALLOW_THREADS
    val = JS_Eval(self->ctx, code, code_len, "<sandbox>", JS_EVAL_RETVAL);
    Py_END_ALLOW_THREADS

    if (self->timed_out) { PyErr_SetString(TimeoutError_ext, "Timeout"); return NULL; }
    if (JS_IsException(val)) {
        JS_GetErrorStr(self->ctx, error_buf, sizeof(error_buf));
        if (strlen(error_buf) == 0) strcpy(error_buf, "Unknown error");
        PyErr_SetString(SandboxError, error_buf);
        return NULL;
    }
    return convert_jsvalue_to_python(self->ctx, val);
}

static PyObject *Sandbox_close(SandboxObject *self, PyObject *Py_UNUSED(ignored))
{
    if (self->ctx) { JS_FreeContext(self->ctx); self->ctx = NULL; }
    if (self->mem_buf) { free(self->mem_buf); self->mem_buf = NULL; }
    Py_RETURN_NONE;
}

static PyMethodDef Sandbox_methods[] = {
    {"execute", (PyCFunction)Sandbox_execute, METH_VARARGS, "Execute JS code"},
    {"close", (PyCFunction)Sandbox_close, METH_NOARGS, "Close sandbox"},
    {NULL}
};

static PyTypeObject SandboxType = {
    PyVarObject_HEAD_INIT(NULL, 0)
    .tp_name = "mquickjs_ext.Sandbox",
    .tp_basicsize = sizeof(SandboxObject),
    .tp_flags = Py_TPFLAGS_DEFAULT,
    .tp_new = PyType_GenericNew,
    .tp_init = (initproc)Sandbox_init,
    .tp_dealloc = (destructor)Sandbox_dealloc,
    .tp_methods = Sandbox_methods,
};

static PyObject *execute_js_func(PyObject *self, PyObject *args, PyObject *kwds)
{
    static char *kwlist[] = {"code", "memory_limit_bytes", "time_limit_ms", NULL};
    const char *code; Py_ssize_t code_len;
    Py_ssize_t memory_limit = 1024 * 1024, time_limit = 1000;
    if (!PyArg_ParseTupleAndKeywords(args, kwds, "s#|nn", kwlist, &code, &code_len, &memory_limit, &time_limit)) return NULL;

    SandboxObject *sandbox = PyObject_New(SandboxObject, &SandboxType);
    if (!sandbox) return NULL;
    sandbox->ctx = NULL; sandbox->mem_buf = NULL;
    sandbox->mem_size = memory_limit; sandbox->time_limit_ms = time_limit; sandbox->timed_out = 0;
    sandbox->mem_buf = malloc(memory_limit);
    if (!sandbox->mem_buf) { Py_DECREF(sandbox); return PyErr_NoMemory(); }
    sandbox->ctx = JS_NewContext(sandbox->mem_buf, memory_limit, &js_stdlib);
    if (!sandbox->ctx) { free(sandbox->mem_buf); Py_DECREF(sandbox); PyErr_SetString(MemoryError_ext, "Failed"); return NULL; }
    JS_SetContextOpaque(sandbox->ctx, sandbox);
    JS_SetInterruptHandler(sandbox->ctx, sandbox_interrupt_handler);
    JS_SetRandomSeed(sandbox->ctx, 12345);

    PyObject *code_arg = Py_BuildValue("(s#)", code, code_len);
    PyObject *result = Sandbox_execute(sandbox, code_arg);
    Py_DECREF(code_arg);
    Py_DECREF(sandbox);
    return result;
}

static PyMethodDef module_methods[] = {
    {"execute_js", (PyCFunction)execute_js_func, METH_VARARGS | METH_KEYWORDS, "Execute JS"},
    {NULL}
};

static struct PyModuleDef mquickjs_ext_module = {
    PyModuleDef_HEAD_INIT, "mquickjs_ext", "mquickjs sandbox", -1, module_methods
};

PyMODINIT_FUNC PyInit_mquickjs_ext(void)
{
    if (PyType_Ready(&SandboxType) < 0) return NULL;
    PyObject *m = PyModule_Create(&mquickjs_ext_module);
    if (!m) return NULL;
    SandboxError = PyErr_NewException("mquickjs_ext.SandboxError", PyExc_Exception, NULL);
    TimeoutError_ext = PyErr_NewException("mquickjs_ext.TimeoutError", SandboxError, NULL);
    MemoryError_ext = PyErr_NewException("mquickjs_ext.MemoryError", SandboxError, NULL);
    Py_INCREF(SandboxError); PyModule_AddObject(m, "SandboxError", SandboxError);
    Py_INCREF(TimeoutError_ext); PyModule_AddObject(m, "TimeoutError", TimeoutError_ext);
    Py_INCREF(MemoryError_ext); PyModule_AddObject(m, "MemoryError", MemoryError_ext);
    Py_INCREF(&SandboxType); PyModule_AddObject(m, "Sandbox", (PyObject *)&SandboxType);
    return m;
}
''')

# Define the extension
ext_module = Extension(
    'mquickjs_ext',
    sources=[
        str(ext_source),
        str(BUILD_DIR / 'mquickjs.c'),
        str(BUILD_DIR / 'dtoa.c'),
        str(BUILD_DIR / 'libm.c'),
        str(BUILD_DIR / 'cutils.c'),
    ],
    include_dirs=[str(BUILD_DIR)],
    define_macros=[('_GNU_SOURCE', None)],
    extra_compile_args=['-Os', '-fno-math-errno', '-fno-trapping-math'],
    extra_link_args=['-lm'],
)

setup(
    name='mquickjs_ext',
    version='0.1.0',
    description='mquickjs JavaScript sandbox C extension',
    ext_modules=[ext_module],
)
