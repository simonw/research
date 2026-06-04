/*
 * pymemchr_c_module.c - Python C Extension for memchr functions
 *
 * This module provides Python bindings for optimized byte and substring
 * search functions implemented in C with SIMD optimizations.
 */

#define PY_SSIZE_T_CLEAN
#include <Python.h>
#include "memchr.h"

/* Helper macro to convert pointer result to Python object */
#define RETURN_OPTIONAL_INDEX(result, base) \
    do { \
        if (result == NULL) { \
            Py_RETURN_NONE; \
        } else { \
            return PyLong_FromSsize_t((Py_ssize_t)(result - base)); \
        } \
    } while (0)

/*
 * memchr(needle, haystack) -> Optional[int]
 */
static PyObject* py_memchr(PyObject* self, PyObject* args) {
    unsigned int needle;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "Iy*", &needle, &haystack)) {
        return NULL;
    }

    if (needle > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needle must be a byte value (0-255)");
        return NULL;
    }

    const uint8_t* result = memchr_find((uint8_t)needle,
                                         (const uint8_t*)haystack.buf,
                                         haystack.len);

    PyObject* ret;
    if (result == NULL) {
        ret = Py_NewRef(Py_None);
    } else {
        ret = PyLong_FromSsize_t((Py_ssize_t)(result - (const uint8_t*)haystack.buf));
    }

    PyBuffer_Release(&haystack);
    return ret;
}

/*
 * memchr2(needle1, needle2, haystack) -> Optional[int]
 */
static PyObject* py_memchr2(PyObject* self, PyObject* args) {
    unsigned int n1, n2;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "IIy*", &n1, &n2, &haystack)) {
        return NULL;
    }

    if (n1 > 255 || n2 > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needles must be byte values (0-255)");
        return NULL;
    }

    const uint8_t* result = memchr2_find((uint8_t)n1, (uint8_t)n2,
                                          (const uint8_t*)haystack.buf,
                                          haystack.len);

    PyObject* ret;
    if (result == NULL) {
        ret = Py_NewRef(Py_None);
    } else {
        ret = PyLong_FromSsize_t((Py_ssize_t)(result - (const uint8_t*)haystack.buf));
    }

    PyBuffer_Release(&haystack);
    return ret;
}

/*
 * memchr3(needle1, needle2, needle3, haystack) -> Optional[int]
 */
static PyObject* py_memchr3(PyObject* self, PyObject* args) {
    unsigned int n1, n2, n3;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "IIIy*", &n1, &n2, &n3, &haystack)) {
        return NULL;
    }

    if (n1 > 255 || n2 > 255 || n3 > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needles must be byte values (0-255)");
        return NULL;
    }

    const uint8_t* result = memchr3_find((uint8_t)n1, (uint8_t)n2, (uint8_t)n3,
                                          (const uint8_t*)haystack.buf,
                                          haystack.len);

    PyObject* ret;
    if (result == NULL) {
        ret = Py_NewRef(Py_None);
    } else {
        ret = PyLong_FromSsize_t((Py_ssize_t)(result - (const uint8_t*)haystack.buf));
    }

    PyBuffer_Release(&haystack);
    return ret;
}

/*
 * memrchr(needle, haystack) -> Optional[int]
 */
static PyObject* py_memrchr(PyObject* self, PyObject* args) {
    unsigned int needle;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "Iy*", &needle, &haystack)) {
        return NULL;
    }

    if (needle > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needle must be a byte value (0-255)");
        return NULL;
    }

    const uint8_t* result = memrchr_find((uint8_t)needle,
                                          (const uint8_t*)haystack.buf,
                                          haystack.len);

    PyObject* ret;
    if (result == NULL) {
        ret = Py_NewRef(Py_None);
    } else {
        ret = PyLong_FromSsize_t((Py_ssize_t)(result - (const uint8_t*)haystack.buf));
    }

    PyBuffer_Release(&haystack);
    return ret;
}

/*
 * memrchr2(needle1, needle2, haystack) -> Optional[int]
 */
static PyObject* py_memrchr2(PyObject* self, PyObject* args) {
    unsigned int n1, n2;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "IIy*", &n1, &n2, &haystack)) {
        return NULL;
    }

    if (n1 > 255 || n2 > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needles must be byte values (0-255)");
        return NULL;
    }

    const uint8_t* result = memrchr2_find((uint8_t)n1, (uint8_t)n2,
                                           (const uint8_t*)haystack.buf,
                                           haystack.len);

    PyObject* ret;
    if (result == NULL) {
        ret = Py_NewRef(Py_None);
    } else {
        ret = PyLong_FromSsize_t((Py_ssize_t)(result - (const uint8_t*)haystack.buf));
    }

    PyBuffer_Release(&haystack);
    return ret;
}

/*
 * memrchr3(needle1, needle2, needle3, haystack) -> Optional[int]
 */
static PyObject* py_memrchr3(PyObject* self, PyObject* args) {
    unsigned int n1, n2, n3;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "IIIy*", &n1, &n2, &n3, &haystack)) {
        return NULL;
    }

    if (n1 > 255 || n2 > 255 || n3 > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needles must be byte values (0-255)");
        return NULL;
    }

    const uint8_t* result = memrchr3_find((uint8_t)n1, (uint8_t)n2, (uint8_t)n3,
                                           (const uint8_t*)haystack.buf,
                                           haystack.len);

    PyObject* ret;
    if (result == NULL) {
        ret = Py_NewRef(Py_None);
    } else {
        ret = PyLong_FromSsize_t((Py_ssize_t)(result - (const uint8_t*)haystack.buf));
    }

    PyBuffer_Release(&haystack);
    return ret;
}

/*
 * memchr_iter(needle, haystack) -> List[int]
 */
static PyObject* py_memchr_iter(PyObject* self, PyObject* args) {
    unsigned int needle;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "Iy*", &needle, &haystack)) {
        return NULL;
    }

    if (needle > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needle must be a byte value (0-255)");
        return NULL;
    }

    PyObject* result_list = PyList_New(0);
    if (result_list == NULL) {
        PyBuffer_Release(&haystack);
        return NULL;
    }

    const uint8_t* base = (const uint8_t*)haystack.buf;
    const uint8_t* ptr = base;
    const uint8_t* end = base + haystack.len;

    while (ptr < end) {
        const uint8_t* found = memchr_find((uint8_t)needle, ptr, end - ptr);
        if (found == NULL) break;

        PyObject* index = PyLong_FromSsize_t((Py_ssize_t)(found - base));
        if (index == NULL || PyList_Append(result_list, index) < 0) {
            Py_XDECREF(index);
            Py_DECREF(result_list);
            PyBuffer_Release(&haystack);
            return NULL;
        }
        Py_DECREF(index);
        ptr = found + 1;
    }

    PyBuffer_Release(&haystack);
    return result_list;
}

/*
 * memchr2_iter(needle1, needle2, haystack) -> List[int]
 */
static PyObject* py_memchr2_iter(PyObject* self, PyObject* args) {
    unsigned int n1, n2;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "IIy*", &n1, &n2, &haystack)) {
        return NULL;
    }

    if (n1 > 255 || n2 > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needles must be byte values (0-255)");
        return NULL;
    }

    PyObject* result_list = PyList_New(0);
    if (result_list == NULL) {
        PyBuffer_Release(&haystack);
        return NULL;
    }

    const uint8_t* base = (const uint8_t*)haystack.buf;
    const uint8_t* ptr = base;
    const uint8_t* end = base + haystack.len;

    while (ptr < end) {
        const uint8_t* found = memchr2_find((uint8_t)n1, (uint8_t)n2, ptr, end - ptr);
        if (found == NULL) break;

        PyObject* index = PyLong_FromSsize_t((Py_ssize_t)(found - base));
        if (index == NULL || PyList_Append(result_list, index) < 0) {
            Py_XDECREF(index);
            Py_DECREF(result_list);
            PyBuffer_Release(&haystack);
            return NULL;
        }
        Py_DECREF(index);
        ptr = found + 1;
    }

    PyBuffer_Release(&haystack);
    return result_list;
}

/*
 * memchr3_iter(needle1, needle2, needle3, haystack) -> List[int]
 */
static PyObject* py_memchr3_iter(PyObject* self, PyObject* args) {
    unsigned int n1, n2, n3;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "IIIy*", &n1, &n2, &n3, &haystack)) {
        return NULL;
    }

    if (n1 > 255 || n2 > 255 || n3 > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needles must be byte values (0-255)");
        return NULL;
    }

    PyObject* result_list = PyList_New(0);
    if (result_list == NULL) {
        PyBuffer_Release(&haystack);
        return NULL;
    }

    const uint8_t* base = (const uint8_t*)haystack.buf;
    const uint8_t* ptr = base;
    const uint8_t* end = base + haystack.len;

    while (ptr < end) {
        const uint8_t* found = memchr3_find((uint8_t)n1, (uint8_t)n2, (uint8_t)n3, ptr, end - ptr);
        if (found == NULL) break;

        PyObject* index = PyLong_FromSsize_t((Py_ssize_t)(found - base));
        if (index == NULL || PyList_Append(result_list, index) < 0) {
            Py_XDECREF(index);
            Py_DECREF(result_list);
            PyBuffer_Release(&haystack);
            return NULL;
        }
        Py_DECREF(index);
        ptr = found + 1;
    }

    PyBuffer_Release(&haystack);
    return result_list;
}

/*
 * memrchr_iter(needle, haystack) -> List[int]
 */
static PyObject* py_memrchr_iter(PyObject* self, PyObject* args) {
    unsigned int needle;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "Iy*", &needle, &haystack)) {
        return NULL;
    }

    if (needle > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needle must be a byte value (0-255)");
        return NULL;
    }

    PyObject* result_list = PyList_New(0);
    if (result_list == NULL) {
        PyBuffer_Release(&haystack);
        return NULL;
    }

    const uint8_t* base = (const uint8_t*)haystack.buf;
    Py_ssize_t remaining = haystack.len;

    while (remaining > 0) {
        const uint8_t* found = memrchr_find((uint8_t)needle, base, remaining);
        if (found == NULL) break;

        PyObject* index = PyLong_FromSsize_t((Py_ssize_t)(found - base));
        if (index == NULL || PyList_Append(result_list, index) < 0) {
            Py_XDECREF(index);
            Py_DECREF(result_list);
            PyBuffer_Release(&haystack);
            return NULL;
        }
        Py_DECREF(index);
        remaining = found - base;
    }

    PyBuffer_Release(&haystack);
    return result_list;
}

/*
 * memrchr2_iter(needle1, needle2, haystack) -> List[int]
 */
static PyObject* py_memrchr2_iter(PyObject* self, PyObject* args) {
    unsigned int n1, n2;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "IIy*", &n1, &n2, &haystack)) {
        return NULL;
    }

    if (n1 > 255 || n2 > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needles must be byte values (0-255)");
        return NULL;
    }

    PyObject* result_list = PyList_New(0);
    if (result_list == NULL) {
        PyBuffer_Release(&haystack);
        return NULL;
    }

    const uint8_t* base = (const uint8_t*)haystack.buf;
    Py_ssize_t remaining = haystack.len;

    while (remaining > 0) {
        const uint8_t* found = memrchr2_find((uint8_t)n1, (uint8_t)n2, base, remaining);
        if (found == NULL) break;

        PyObject* index = PyLong_FromSsize_t((Py_ssize_t)(found - base));
        if (index == NULL || PyList_Append(result_list, index) < 0) {
            Py_XDECREF(index);
            Py_DECREF(result_list);
            PyBuffer_Release(&haystack);
            return NULL;
        }
        Py_DECREF(index);
        remaining = found - base;
    }

    PyBuffer_Release(&haystack);
    return result_list;
}

/*
 * memrchr3_iter(needle1, needle2, needle3, haystack) -> List[int]
 */
static PyObject* py_memrchr3_iter(PyObject* self, PyObject* args) {
    unsigned int n1, n2, n3;
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "IIIy*", &n1, &n2, &n3, &haystack)) {
        return NULL;
    }

    if (n1 > 255 || n2 > 255 || n3 > 255) {
        PyBuffer_Release(&haystack);
        PyErr_SetString(PyExc_ValueError, "needles must be byte values (0-255)");
        return NULL;
    }

    PyObject* result_list = PyList_New(0);
    if (result_list == NULL) {
        PyBuffer_Release(&haystack);
        return NULL;
    }

    const uint8_t* base = (const uint8_t*)haystack.buf;
    Py_ssize_t remaining = haystack.len;

    while (remaining > 0) {
        const uint8_t* found = memrchr3_find((uint8_t)n1, (uint8_t)n2, (uint8_t)n3, base, remaining);
        if (found == NULL) break;

        PyObject* index = PyLong_FromSsize_t((Py_ssize_t)(found - base));
        if (index == NULL || PyList_Append(result_list, index) < 0) {
            Py_XDECREF(index);
            Py_DECREF(result_list);
            PyBuffer_Release(&haystack);
            return NULL;
        }
        Py_DECREF(index);
        remaining = found - base;
    }

    PyBuffer_Release(&haystack);
    return result_list;
}

/*
 * memmem_find(needle, haystack) -> Optional[int]
 */
static PyObject* py_memmem_find(PyObject* self, PyObject* args) {
    Py_buffer needle, haystack;

    if (!PyArg_ParseTuple(args, "y*y*", &needle, &haystack)) {
        return NULL;
    }

    const uint8_t* result = memmem_find((const uint8_t*)needle.buf, needle.len,
                                         (const uint8_t*)haystack.buf, haystack.len);

    PyObject* ret;
    if (result == NULL) {
        ret = Py_NewRef(Py_None);
    } else {
        ret = PyLong_FromSsize_t((Py_ssize_t)(result - (const uint8_t*)haystack.buf));
    }

    PyBuffer_Release(&needle);
    PyBuffer_Release(&haystack);
    return ret;
}

/*
 * memmem_rfind(needle, haystack) -> Optional[int]
 */
static PyObject* py_memmem_rfind(PyObject* self, PyObject* args) {
    Py_buffer needle, haystack;

    if (!PyArg_ParseTuple(args, "y*y*", &needle, &haystack)) {
        return NULL;
    }

    const uint8_t* result = memmem_rfind((const uint8_t*)needle.buf, needle.len,
                                          (const uint8_t*)haystack.buf, haystack.len);

    PyObject* ret;
    if (result == NULL) {
        ret = Py_NewRef(Py_None);
    } else {
        ret = PyLong_FromSsize_t((Py_ssize_t)(result - (const uint8_t*)haystack.buf));
    }

    PyBuffer_Release(&needle);
    PyBuffer_Release(&haystack);
    return ret;
}

/*
 * memmem_find_iter(needle, haystack) -> List[int]
 */
static PyObject* py_memmem_find_iter(PyObject* self, PyObject* args) {
    Py_buffer needle, haystack;

    if (!PyArg_ParseTuple(args, "y*y*", &needle, &haystack)) {
        return NULL;
    }

    PyObject* result_list = PyList_New(0);
    if (result_list == NULL) {
        PyBuffer_Release(&needle);
        PyBuffer_Release(&haystack);
        return NULL;
    }

    /* Empty needle - return empty list */
    if (needle.len == 0) {
        PyBuffer_Release(&needle);
        PyBuffer_Release(&haystack);
        return result_list;
    }

    const uint8_t* base = (const uint8_t*)haystack.buf;
    const uint8_t* ptr = base;
    const uint8_t* end = base + haystack.len;
    const uint8_t* needle_ptr = (const uint8_t*)needle.buf;
    size_t needle_len = needle.len;

    while (ptr + needle_len <= end) {
        const uint8_t* found = memmem_find(needle_ptr, needle_len, ptr, end - ptr);
        if (found == NULL) break;

        PyObject* index = PyLong_FromSsize_t((Py_ssize_t)(found - base));
        if (index == NULL || PyList_Append(result_list, index) < 0) {
            Py_XDECREF(index);
            Py_DECREF(result_list);
            PyBuffer_Release(&needle);
            PyBuffer_Release(&haystack);
            return NULL;
        }
        Py_DECREF(index);
        ptr = found + needle_len;  /* Non-overlapping matches */
    }

    PyBuffer_Release(&needle);
    PyBuffer_Release(&haystack);
    return result_list;
}

/*
 * Finder class - precompiled substring finder
 */
typedef struct {
    PyObject_HEAD
    PyObject* needle_bytes;
} FinderObject;

static void Finder_dealloc(FinderObject* self) {
    Py_XDECREF(self->needle_bytes);
    Py_TYPE(self)->tp_free((PyObject*)self);
}

static PyObject* Finder_new(PyTypeObject* type, PyObject* args, PyObject* kwds) {
    FinderObject* self;
    self = (FinderObject*)type->tp_alloc(type, 0);
    if (self != NULL) {
        self->needle_bytes = NULL;
    }
    return (PyObject*)self;
}

static int Finder_init(FinderObject* self, PyObject* args, PyObject* kwds) {
    Py_buffer needle;

    if (!PyArg_ParseTuple(args, "y*", &needle)) {
        return -1;
    }

    /* Store a copy of the needle */
    self->needle_bytes = PyBytes_FromStringAndSize((const char*)needle.buf, needle.len);
    PyBuffer_Release(&needle);

    if (self->needle_bytes == NULL) {
        return -1;
    }

    return 0;
}

static PyObject* Finder_find(FinderObject* self, PyObject* args) {
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "y*", &haystack)) {
        return NULL;
    }

    const uint8_t* needle = (const uint8_t*)PyBytes_AS_STRING(self->needle_bytes);
    Py_ssize_t needle_len = PyBytes_GET_SIZE(self->needle_bytes);

    const uint8_t* result = memmem_find(needle, needle_len,
                                         (const uint8_t*)haystack.buf, haystack.len);

    PyObject* ret;
    if (result == NULL) {
        ret = Py_NewRef(Py_None);
    } else {
        ret = PyLong_FromSsize_t((Py_ssize_t)(result - (const uint8_t*)haystack.buf));
    }

    PyBuffer_Release(&haystack);
    return ret;
}

static PyObject* Finder_find_iter(FinderObject* self, PyObject* args) {
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "y*", &haystack)) {
        return NULL;
    }

    PyObject* result_list = PyList_New(0);
    if (result_list == NULL) {
        PyBuffer_Release(&haystack);
        return NULL;
    }

    const uint8_t* needle = (const uint8_t*)PyBytes_AS_STRING(self->needle_bytes);
    Py_ssize_t needle_len = PyBytes_GET_SIZE(self->needle_bytes);

    if (needle_len == 0) {
        PyBuffer_Release(&haystack);
        return result_list;
    }

    const uint8_t* base = (const uint8_t*)haystack.buf;
    const uint8_t* ptr = base;
    const uint8_t* end = base + haystack.len;

    while (ptr + needle_len <= end) {
        const uint8_t* found = memmem_find(needle, needle_len, ptr, end - ptr);
        if (found == NULL) break;

        PyObject* index = PyLong_FromSsize_t((Py_ssize_t)(found - base));
        if (index == NULL || PyList_Append(result_list, index) < 0) {
            Py_XDECREF(index);
            Py_DECREF(result_list);
            PyBuffer_Release(&haystack);
            return NULL;
        }
        Py_DECREF(index);
        ptr = found + needle_len;
    }

    PyBuffer_Release(&haystack);
    return result_list;
}

static PyObject* Finder_needle(FinderObject* self, PyObject* Py_UNUSED(ignored)) {
    /* Return a list of bytes (to match Rust API which returns Vec<u8>) */
    PyObject* result = PyList_New(PyBytes_GET_SIZE(self->needle_bytes));
    if (result == NULL) return NULL;

    const uint8_t* data = (const uint8_t*)PyBytes_AS_STRING(self->needle_bytes);
    Py_ssize_t len = PyBytes_GET_SIZE(self->needle_bytes);

    for (Py_ssize_t i = 0; i < len; i++) {
        PyObject* byte_val = PyLong_FromLong(data[i]);
        if (byte_val == NULL) {
            Py_DECREF(result);
            return NULL;
        }
        PyList_SET_ITEM(result, i, byte_val);
    }

    return result;
}

static PyMethodDef Finder_methods[] = {
    {"find", (PyCFunction)Finder_find, METH_VARARGS,
     "Find the first occurrence of the needle in the haystack."},
    {"find_iter", (PyCFunction)Finder_find_iter, METH_VARARGS,
     "Find all occurrences of the needle in the haystack."},
    {"needle", (PyCFunction)Finder_needle, METH_NOARGS,
     "Get the needle as a list of bytes."},
    {NULL}
};

static PyTypeObject FinderType = {
    PyVarObject_HEAD_INIT(NULL, 0)
    .tp_name = "pymemchr_c._pymemchr_c.Finder",
    .tp_doc = "Precompiled substring finder for repeated searches.",
    .tp_basicsize = sizeof(FinderObject),
    .tp_itemsize = 0,
    .tp_flags = Py_TPFLAGS_DEFAULT | Py_TPFLAGS_BASETYPE,
    .tp_new = Finder_new,
    .tp_init = (initproc)Finder_init,
    .tp_dealloc = (destructor)Finder_dealloc,
    .tp_methods = Finder_methods,
};

/*
 * FinderRev class - precompiled reverse substring finder
 */
typedef struct {
    PyObject_HEAD
    PyObject* needle_bytes;
} FinderRevObject;

static void FinderRev_dealloc(FinderRevObject* self) {
    Py_XDECREF(self->needle_bytes);
    Py_TYPE(self)->tp_free((PyObject*)self);
}

static PyObject* FinderRev_new(PyTypeObject* type, PyObject* args, PyObject* kwds) {
    FinderRevObject* self;
    self = (FinderRevObject*)type->tp_alloc(type, 0);
    if (self != NULL) {
        self->needle_bytes = NULL;
    }
    return (PyObject*)self;
}

static int FinderRev_init(FinderRevObject* self, PyObject* args, PyObject* kwds) {
    Py_buffer needle;

    if (!PyArg_ParseTuple(args, "y*", &needle)) {
        return -1;
    }

    self->needle_bytes = PyBytes_FromStringAndSize((const char*)needle.buf, needle.len);
    PyBuffer_Release(&needle);

    if (self->needle_bytes == NULL) {
        return -1;
    }

    return 0;
}

static PyObject* FinderRev_rfind(FinderRevObject* self, PyObject* args) {
    Py_buffer haystack;

    if (!PyArg_ParseTuple(args, "y*", &haystack)) {
        return NULL;
    }

    const uint8_t* needle = (const uint8_t*)PyBytes_AS_STRING(self->needle_bytes);
    Py_ssize_t needle_len = PyBytes_GET_SIZE(self->needle_bytes);

    const uint8_t* result = memmem_rfind(needle, needle_len,
                                          (const uint8_t*)haystack.buf, haystack.len);

    PyObject* ret;
    if (result == NULL) {
        ret = Py_NewRef(Py_None);
    } else {
        ret = PyLong_FromSsize_t((Py_ssize_t)(result - (const uint8_t*)haystack.buf));
    }

    PyBuffer_Release(&haystack);
    return ret;
}

static PyObject* FinderRev_needle(FinderRevObject* self, PyObject* Py_UNUSED(ignored)) {
    PyObject* result = PyList_New(PyBytes_GET_SIZE(self->needle_bytes));
    if (result == NULL) return NULL;

    const uint8_t* data = (const uint8_t*)PyBytes_AS_STRING(self->needle_bytes);
    Py_ssize_t len = PyBytes_GET_SIZE(self->needle_bytes);

    for (Py_ssize_t i = 0; i < len; i++) {
        PyObject* byte_val = PyLong_FromLong(data[i]);
        if (byte_val == NULL) {
            Py_DECREF(result);
            return NULL;
        }
        PyList_SET_ITEM(result, i, byte_val);
    }

    return result;
}

static PyMethodDef FinderRev_methods[] = {
    {"rfind", (PyCFunction)FinderRev_rfind, METH_VARARGS,
     "Find the last occurrence of the needle in the haystack."},
    {"needle", (PyCFunction)FinderRev_needle, METH_NOARGS,
     "Get the needle as a list of bytes."},
    {NULL}
};

static PyTypeObject FinderRevType = {
    PyVarObject_HEAD_INIT(NULL, 0)
    .tp_name = "pymemchr_c._pymemchr_c.FinderRev",
    .tp_doc = "Precompiled reverse substring finder.",
    .tp_basicsize = sizeof(FinderRevObject),
    .tp_itemsize = 0,
    .tp_flags = Py_TPFLAGS_DEFAULT | Py_TPFLAGS_BASETYPE,
    .tp_new = FinderRev_new,
    .tp_init = (initproc)FinderRev_init,
    .tp_dealloc = (destructor)FinderRev_dealloc,
    .tp_methods = FinderRev_methods,
};

/*
 * Module method definitions
 */
static PyMethodDef module_methods[] = {
    {"memchr", py_memchr, METH_VARARGS,
     "Find the first occurrence of a byte in a haystack."},
    {"memchr2", py_memchr2, METH_VARARGS,
     "Find the first occurrence of either of two bytes in a haystack."},
    {"memchr3", py_memchr3, METH_VARARGS,
     "Find the first occurrence of any of three bytes in a haystack."},
    {"memrchr", py_memrchr, METH_VARARGS,
     "Find the last occurrence of a byte in a haystack."},
    {"memrchr2", py_memrchr2, METH_VARARGS,
     "Find the last occurrence of either of two bytes in a haystack."},
    {"memrchr3", py_memrchr3, METH_VARARGS,
     "Find the last occurrence of any of three bytes in a haystack."},
    {"memchr_iter", py_memchr_iter, METH_VARARGS,
     "Find all occurrences of a byte in a haystack."},
    {"memchr2_iter", py_memchr2_iter, METH_VARARGS,
     "Find all occurrences of either of two bytes in a haystack."},
    {"memchr3_iter", py_memchr3_iter, METH_VARARGS,
     "Find all occurrences of any of three bytes in a haystack."},
    {"memrchr_iter", py_memrchr_iter, METH_VARARGS,
     "Find all occurrences of a byte in a haystack in reverse order."},
    {"memrchr2_iter", py_memrchr2_iter, METH_VARARGS,
     "Find all occurrences of either of two bytes in a haystack in reverse order."},
    {"memrchr3_iter", py_memrchr3_iter, METH_VARARGS,
     "Find all occurrences of any of three bytes in a haystack in reverse order."},
    {"memmem_find", py_memmem_find, METH_VARARGS,
     "Find the first occurrence of a substring in a haystack."},
    {"memmem_rfind", py_memmem_rfind, METH_VARARGS,
     "Find the last occurrence of a substring in a haystack."},
    {"memmem_find_iter", py_memmem_find_iter, METH_VARARGS,
     "Find all occurrences of a substring in a haystack."},
    {NULL, NULL, 0, NULL}
};

/*
 * Module definition
 */
static struct PyModuleDef pymemchr_c_module = {
    PyModuleDef_HEAD_INIT,
    "_pymemchr_c",
    "Python bindings for optimized byte and substring search functions in C.",
    -1,
    module_methods
};

/*
 * Module initialization
 */
PyMODINIT_FUNC PyInit__pymemchr_c(void) {
    PyObject* m;

    if (PyType_Ready(&FinderType) < 0)
        return NULL;
    if (PyType_Ready(&FinderRevType) < 0)
        return NULL;

    m = PyModule_Create(&pymemchr_c_module);
    if (m == NULL)
        return NULL;

    Py_INCREF(&FinderType);
    if (PyModule_AddObject(m, "Finder", (PyObject*)&FinderType) < 0) {
        Py_DECREF(&FinderType);
        Py_DECREF(m);
        return NULL;
    }

    Py_INCREF(&FinderRevType);
    if (PyModule_AddObject(m, "FinderRev", (PyObject*)&FinderRevType) < 0) {
        Py_DECREF(&FinderRevType);
        Py_DECREF(&FinderType);
        Py_DECREF(m);
        return NULL;
    }

    return m;
}
