#include <Python.h>
#include <sqlite3.h>
#include <stdint.h>
#include <time.h>

typedef struct {
    int64_t start_ms;
    int64_t timeout_ms;
} TimeoutContext;

static int64_t monotonic_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ((int64_t)ts.tv_sec * 1000) + (ts.tv_nsec / 1000000);
}

static int progress_handler(void *data) {
    TimeoutContext *ctx = (TimeoutContext *)data;
    int64_t now_ms = monotonic_ms();
    if (now_ms - ctx->start_ms > ctx->timeout_ms) {
        return 1;
    }
    return 0;
}

static PyObject *execute_with_timeout(PyObject *self, PyObject *args) {
    const char *db_path = NULL;
    const char *sql = NULL;
    long timeout_ms = 0;
    sqlite3 *db = NULL;
    sqlite3_stmt *stmt = NULL;
    int rc = 0;
    PyObject *rows = NULL;
    PyObject *row = NULL;

    if (!PyArg_ParseTuple(args, "ssl", &db_path, &sql, &timeout_ms)) {
        return NULL;
    }

    if (timeout_ms < 0) {
        PyErr_SetString(PyExc_ValueError, "timeout_ms must be >= 0");
        return NULL;
    }

    rc = sqlite3_open_v2(db_path, &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE, NULL);
    if (rc != SQLITE_OK) {
        PyErr_Format(PyExc_RuntimeError, "failed to open database: %s", sqlite3_errmsg(db));
        sqlite3_close(db);
        return NULL;
    }

    rc = sqlite3_prepare_v2(db, sql, -1, &stmt, NULL);
    if (rc != SQLITE_OK) {
        PyErr_Format(PyExc_RuntimeError, "failed to prepare statement: %s", sqlite3_errmsg(db));
        sqlite3_close(db);
        return NULL;
    }

    TimeoutContext ctx;
    if (timeout_ms > 0) {
        ctx.start_ms = monotonic_ms();
        ctx.timeout_ms = timeout_ms;
        sqlite3_progress_handler(db, 1000, progress_handler, &ctx);
    }

    rows = PyList_New(0);
    if (rows == NULL) {
        goto cleanup;
    }

    while ((rc = sqlite3_step(stmt)) == SQLITE_ROW) {
        int column_count = sqlite3_column_count(stmt);
        row = PyTuple_New(column_count);
        if (row == NULL) {
            goto cleanup;
        }

        for (int i = 0; i < column_count; i++) {
            PyObject *value = NULL;
            int col_type = sqlite3_column_type(stmt, i);
            switch (col_type) {
                case SQLITE_INTEGER:
                    value = PyLong_FromLongLong(sqlite3_column_int64(stmt, i));
                    break;
                case SQLITE_FLOAT:
                    value = PyFloat_FromDouble(sqlite3_column_double(stmt, i));
                    break;
                case SQLITE_TEXT: {
                    const unsigned char *text = sqlite3_column_text(stmt, i);
                    int bytes = sqlite3_column_bytes(stmt, i);
                    value = PyUnicode_FromStringAndSize((const char *)text, bytes);
                    break;
                }
                case SQLITE_BLOB: {
                    const void *blob = sqlite3_column_blob(stmt, i);
                    int bytes = sqlite3_column_bytes(stmt, i);
                    value = PyBytes_FromStringAndSize((const char *)blob, bytes);
                    break;
                }
                case SQLITE_NULL:
                default:
                    Py_INCREF(Py_None);
                    value = Py_None;
                    break;
            }

            if (value == NULL) {
                goto cleanup;
            }
            PyTuple_SET_ITEM(row, i, value);
        }

        if (PyList_Append(rows, row) < 0) {
            goto cleanup;
        }
        Py_DECREF(row);
        row = NULL;
    }

    if (rc == SQLITE_INTERRUPT) {
        PyErr_SetString(PyExc_TimeoutError, "query timed out");
        goto cleanup;
    }

    if (rc != SQLITE_DONE) {
        PyErr_Format(PyExc_RuntimeError, "query failed: %s", sqlite3_errmsg(db));
        goto cleanup;
    }

    sqlite3_progress_handler(db, 0, NULL, NULL);
    sqlite3_finalize(stmt);
    sqlite3_close(db);
    return rows;

cleanup:
    sqlite3_progress_handler(db, 0, NULL, NULL);
    if (row != NULL) {
        Py_DECREF(row);
    }
    Py_XDECREF(rows);
    if (stmt != NULL) {
        sqlite3_finalize(stmt);
    }
    sqlite3_close(db);
    return NULL;
}

static PyMethodDef module_methods[] = {
    {"execute_with_timeout", execute_with_timeout, METH_VARARGS, "Execute SQL with a timeout in milliseconds."},
    {NULL, NULL, 0, NULL}
};

static struct PyModuleDef moduledef = {
    PyModuleDef_HEAD_INIT,
    "_time_limit",
    NULL,
    -1,
    module_methods,
};

PyMODINIT_FUNC PyInit__time_limit(void) {
    return PyModule_Create(&moduledef);
}
