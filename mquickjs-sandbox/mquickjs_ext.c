/*
 * mquickjs Python C Extension
 *
 * This module provides direct Python bindings to mquickjs.
 */

#define PY_SSIZE_T_CLEAN
#include <Python.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <stdint.h>

#include "cutils.h"
#include "mquickjs.h"
#include "mqjs_stdlib.h"

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

/* Interrupt handler for time limit */
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
    if (self->ctx)
        JS_FreeContext(self->ctx);
    if (self->mem_buf)
        free(self->mem_buf);
    Py_TYPE(self)->tp_free((PyObject *)self);
}

static int Sandbox_init(SandboxObject *self, PyObject *args, PyObject *kwds)
{
    static char *kwlist[] = {"memory_limit_bytes", "time_limit_ms", NULL};
    Py_ssize_t memory_limit = 1024 * 1024;  /* 1MB default */
    Py_ssize_t time_limit = 1000;  /* 1 second default */

    if (!PyArg_ParseTupleAndKeywords(args, kwds, "|nn", kwlist,
                                     &memory_limit, &time_limit))
        return -1;

    if (memory_limit < 8192) {
        PyErr_SetString(PyExc_ValueError, "memory_limit_bytes must be at least 8192");
        return -1;
    }

    self->mem_size = memory_limit;
    self->time_limit_ms = time_limit;
    self->timed_out = 0;

    self->mem_buf = malloc(memory_limit);
    if (!self->mem_buf) {
        PyErr_NoMemory();
        return -1;
    }

    self->ctx = JS_NewContext(self->mem_buf, memory_limit, &js_stdlib);
    if (!self->ctx) {
        free(self->mem_buf);
        self->mem_buf = NULL;
        PyErr_SetString(MemoryError_ext, "Failed to create JavaScript context");
        return -1;
    }

    JS_SetContextOpaque(self->ctx, self);
    JS_SetInterruptHandler(self->ctx, sandbox_interrupt_handler);
    JS_SetRandomSeed(self->ctx, 12345);

    return 0;
}

static PyObject *convert_jsvalue_to_python(JSContext *ctx, JSValue val)
{
    if (JS_IsUndefined(val) || JS_IsNull(val)) {
        Py_RETURN_NONE;
    } else if (JS_IsBool(val)) {
        if (val == JS_TRUE)
            Py_RETURN_TRUE;
        else
            Py_RETURN_FALSE;
    } else if (JS_IsInt(val)) {
        return PyLong_FromLong(JS_VALUE_GET_INT(val));
    } else if (JS_IsNumber(ctx, val)) {
        double d;
        JS_ToNumber(ctx, &d, val);
        return PyFloat_FromDouble(d);
    } else if (JS_IsString(ctx, val)) {
        JSCStringBuf buf;
        size_t len;
        const char *str = JS_ToCStringLen(ctx, &len, val, &buf);
        if (str) {
            return PyUnicode_FromStringAndSize(str, len);
        }
        Py_RETURN_NONE;
    } else {
        /* For objects/arrays, convert to string */
        JSValue str_val = JS_ToString(ctx, val);
        if (JS_IsString(ctx, str_val)) {
            JSCStringBuf buf;
            size_t len;
            const char *str = JS_ToCStringLen(ctx, &len, str_val, &buf);
            if (str) {
                return PyUnicode_FromStringAndSize(str, len);
            }
        }
        return PyUnicode_FromString("[object]");
    }
}

static PyObject *Sandbox_execute(SandboxObject *self, PyObject *args)
{
    const char *code;
    Py_ssize_t code_len;
    JSValue val;
    char error_buf[4096];

    if (!PyArg_ParseTuple(args, "s#", &code, &code_len))
        return NULL;

    if (!self->ctx) {
        PyErr_SetString(SandboxError, "Sandbox context is closed");
        return NULL;
    }

    self->start_time_ms = get_time_ms();
    self->timed_out = 0;

    Py_BEGIN_ALLOW_THREADS
    val = JS_Eval(self->ctx, code, code_len, "<sandbox>", JS_EVAL_RETVAL);
    Py_END_ALLOW_THREADS

    if (self->timed_out) {
        PyErr_SetString(TimeoutError_ext, "Execution timeout");
        return NULL;
    }

    if (JS_IsException(val)) {
        JS_GetErrorStr(self->ctx, error_buf, sizeof(error_buf));
        if (strlen(error_buf) == 0) {
            strcpy(error_buf, "Unknown JavaScript error");
        }
        PyErr_SetString(SandboxError, error_buf);
        return NULL;
    }

    return convert_jsvalue_to_python(self->ctx, val);
}

static PyObject *Sandbox_close(SandboxObject *self, PyObject *Py_UNUSED(ignored))
{
    if (self->ctx) {
        JS_FreeContext(self->ctx);
        self->ctx = NULL;
    }
    if (self->mem_buf) {
        free(self->mem_buf);
        self->mem_buf = NULL;
    }
    Py_RETURN_NONE;
}

static PyMethodDef Sandbox_methods[] = {
    {"execute", (PyCFunction)Sandbox_execute, METH_VARARGS,
     "Execute JavaScript code and return the result"},
    {"close", (PyCFunction)Sandbox_close, METH_NOARGS,
     "Close the sandbox context"},
    {NULL}
};

static PyTypeObject SandboxType = {
    PyVarObject_HEAD_INIT(NULL, 0)
    .tp_name = "mquickjs_ext.Sandbox",
    .tp_doc = "mquickjs JavaScript sandbox",
    .tp_basicsize = sizeof(SandboxObject),
    .tp_itemsize = 0,
    .tp_flags = Py_TPFLAGS_DEFAULT | Py_TPFLAGS_BASETYPE,
    .tp_new = PyType_GenericNew,
    .tp_init = (initproc)Sandbox_init,
    .tp_dealloc = (destructor)Sandbox_dealloc,
    .tp_methods = Sandbox_methods,
};

static PyObject *execute_js(PyObject *self, PyObject *args, PyObject *kwds)
{
    static char *kwlist[] = {"code", "memory_limit_bytes", "time_limit_ms", NULL};
    const char *code;
    Py_ssize_t code_len;
    Py_ssize_t memory_limit = 1024 * 1024;
    Py_ssize_t time_limit = 1000;
    SandboxObject *sandbox;
    PyObject *result;

    if (!PyArg_ParseTupleAndKeywords(args, kwds, "s#|nn", kwlist,
                                     &code, &code_len, &memory_limit, &time_limit))
        return NULL;

    /* Create temporary sandbox */
    sandbox = (SandboxObject *)PyObject_CallObject((PyObject *)&SandboxType, NULL);
    if (!sandbox)
        return NULL;

    /* Initialize it */
    sandbox->mem_size = memory_limit;
    sandbox->time_limit_ms = time_limit;
    sandbox->mem_buf = malloc(memory_limit);
    if (!sandbox->mem_buf) {
        Py_DECREF(sandbox);
        return PyErr_NoMemory();
    }
    sandbox->ctx = JS_NewContext(sandbox->mem_buf, memory_limit, &js_stdlib);
    if (!sandbox->ctx) {
        free(sandbox->mem_buf);
        Py_DECREF(sandbox);
        PyErr_SetString(MemoryError_ext, "Failed to create context");
        return NULL;
    }
    JS_SetContextOpaque(sandbox->ctx, sandbox);
    JS_SetInterruptHandler(sandbox->ctx, sandbox_interrupt_handler);
    JS_SetRandomSeed(sandbox->ctx, 12345);

    /* Execute */
    PyObject *code_arg = Py_BuildValue("(s#)", code, code_len);
    result = Sandbox_execute(sandbox, code_arg);
    Py_DECREF(code_arg);

    /* Cleanup */
    Py_DECREF(sandbox);

    return result;
}

static PyMethodDef module_methods[] = {
    {"execute_js", (PyCFunction)execute_js, METH_VARARGS | METH_KEYWORDS,
     "Execute JavaScript code in a sandbox"},
    {NULL}
};

static struct PyModuleDef mquickjs_ext_module = {
    PyModuleDef_HEAD_INIT,
    "mquickjs_ext",
    "mquickjs JavaScript sandbox extension",
    -1,
    module_methods
};

PyMODINIT_FUNC PyInit_mquickjs_ext(void)
{
    PyObject *m;

    if (PyType_Ready(&SandboxType) < 0)
        return NULL;

    m = PyModule_Create(&mquickjs_ext_module);
    if (m == NULL)
        return NULL;

    /* Create exception types */
    SandboxError = PyErr_NewException("mquickjs_ext.SandboxError", PyExc_Exception, NULL);
    Py_INCREF(SandboxError);
    PyModule_AddObject(m, "SandboxError", SandboxError);

    TimeoutError_ext = PyErr_NewException("mquickjs_ext.TimeoutError", SandboxError, NULL);
    Py_INCREF(TimeoutError_ext);
    PyModule_AddObject(m, "TimeoutError", TimeoutError_ext);

    MemoryError_ext = PyErr_NewException("mquickjs_ext.MemoryError", SandboxError, NULL);
    Py_INCREF(MemoryError_ext);
    PyModule_AddObject(m, "MemoryError", MemoryError_ext);

    Py_INCREF(&SandboxType);
    PyModule_AddObject(m, "Sandbox", (PyObject *)&SandboxType);

    return m;
}
