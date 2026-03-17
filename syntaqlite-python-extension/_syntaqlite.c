/*
 * Python C extension module for syntaqlite.
 *
 * Exposes: parse, format_sql, validate, tokenize
 * Links against libsyntaqlite.a + libsyntaqlite_engine.a + libsyntaqlite_sqlite.a
 */

#define PY_SSIZE_T_CLEAN
#include <Python.h>
#include "syntaqlite/parser.h"
#include "syntaqlite/tokenizer.h"
#include "syntaqlite/formatter.h"
#include "syntaqlite/validation.h"

/* Custom exception for format errors */
static PyObject *FormatError;

/* ─── parse ─────────────────────────────────────────────────────────── */

static PyObject *
syntaqlite_py_parse(PyObject *self, PyObject *args)
{
    const char *sql;
    Py_ssize_t sql_len;

    if (!PyArg_ParseTuple(args, "s#", &sql, &sql_len))
        return NULL;

    PyObject *result_list = PyList_New(0);
    if (!result_list)
        return NULL;

    SyntaqliteParser *p = syntaqlite_parser_create(NULL);
    if (!p) {
        Py_DECREF(result_list);
        return PyErr_NoMemory();
    }

    syntaqlite_parser_reset(p, sql, (uint32_t)sql_len);

    for (;;) {
        int32_t rc = syntaqlite_parser_next(p);
        if (rc == SYNTAQLITE_PARSE_DONE)
            break;

        PyObject *stmt_dict = PyDict_New();
        if (!stmt_dict) {
            syntaqlite_parser_destroy(p);
            Py_DECREF(result_list);
            return NULL;
        }

        if (rc == SYNTAQLITE_PARSE_OK) {
            PyDict_SetItemString(stmt_dict, "ok", Py_True);

            uint32_t root = syntaqlite_result_root(p);
            char *dump = syntaqlite_dump_node(p, root, 0);
            if (dump) {
                PyObject *ast_str = PyUnicode_FromString(dump);
                free(dump);
                if (ast_str) {
                    PyDict_SetItemString(stmt_dict, "ast", ast_str);
                    Py_DECREF(ast_str);
                }
            } else {
                PyDict_SetItemString(stmt_dict, "ast", Py_None);
            }
            PyDict_SetItemString(stmt_dict, "error", Py_None);
        } else {
            /* SYNTAQLITE_PARSE_ERROR */
            PyDict_SetItemString(stmt_dict, "ok", Py_False);
            PyDict_SetItemString(stmt_dict, "ast", Py_None);

            const char *err = syntaqlite_result_error_msg(p);
            if (err) {
                PyObject *err_str = PyUnicode_FromString(err);
                if (err_str) {
                    PyDict_SetItemString(stmt_dict, "error", err_str);
                    Py_DECREF(err_str);
                }
            } else {
                PyObject *err_str = PyUnicode_FromString("unknown parse error");
                if (err_str) {
                    PyDict_SetItemString(stmt_dict, "error", err_str);
                    Py_DECREF(err_str);
                }
            }

            uint32_t err_off = syntaqlite_result_error_offset(p);
            uint32_t err_len = syntaqlite_result_error_length(p);
            PyObject *off_obj = PyLong_FromUnsignedLong(err_off);
            PyObject *len_obj = PyLong_FromUnsignedLong(err_len);
            if (off_obj) {
                PyDict_SetItemString(stmt_dict, "error_offset", off_obj);
                Py_DECREF(off_obj);
            }
            if (len_obj) {
                PyDict_SetItemString(stmt_dict, "error_length", len_obj);
                Py_DECREF(len_obj);
            }

            /* Check for recovery tree */
            uint32_t recovery = syntaqlite_result_recovery_root(p);
            if (recovery == SYNTAQLITE_NULL_NODE)
            {
                PyList_Append(result_list, stmt_dict);
                Py_DECREF(stmt_dict);
                break;
            }
        }

        PyList_Append(result_list, stmt_dict);
        Py_DECREF(stmt_dict);
    }

    syntaqlite_parser_destroy(p);
    return result_list;
}

/* ─── format_sql ────────────────────────────────────────────────────── */

static PyObject *
syntaqlite_py_format_sql(PyObject *self, PyObject *args, PyObject *kwargs)
{
    const char *sql;
    Py_ssize_t sql_len;
    unsigned int line_width = 80;
    unsigned int indent_width = 2;
    const char *keyword_case_str = "upper";
    int semicolons = 1;

    static char *kwlist[] = {"sql", "line_width", "indent_width",
                             "keyword_case", "semicolons", NULL};

    if (!PyArg_ParseTupleAndKeywords(args, kwargs, "s#|IIsp", kwlist,
                                     &sql, &sql_len,
                                     &line_width, &indent_width,
                                     &keyword_case_str, &semicolons))
        return NULL;

    SyntaqliteFormatConfig config;
    config.line_width = line_width;
    config.indent_width = indent_width;
    config.semicolons = semicolons ? 1 : 0;

    if (strcmp(keyword_case_str, "lower") == 0)
        config.keyword_case = SYNTAQLITE_KEYWORD_LOWER;
    else
        config.keyword_case = SYNTAQLITE_KEYWORD_UPPER;

    SyntaqliteFormatter *f = syntaqlite_formatter_create_sqlite_with_config(&config);
    if (!f)
        return PyErr_NoMemory();

    int32_t rc = syntaqlite_formatter_format(f, sql, (uint32_t)sql_len);
    if (rc != SYNTAQLITE_FORMAT_OK) {
        const char *err = syntaqlite_formatter_error_msg(f);
        PyObject *err_str = PyUnicode_FromString(err ? err : "format error");
        if (err_str) {
            PyErr_SetObject(FormatError, err_str);
            Py_DECREF(err_str);
        }
        syntaqlite_formatter_destroy(f);
        return NULL;
    }

    const char *output = syntaqlite_formatter_output(f);
    uint32_t output_len = syntaqlite_formatter_output_len(f);

    PyObject *result = PyUnicode_FromStringAndSize(output, output_len);
    syntaqlite_formatter_destroy(f);
    return result;
}

/* ─── validate ──────────────────────────────────────────────────────── */

static PyObject *
syntaqlite_py_validate(PyObject *self, PyObject *args, PyObject *kwargs)
{
    const char *sql;
    Py_ssize_t sql_len;
    PyObject *tables_list = NULL;
    int render = 0;

    static char *kwlist[] = {"sql", "tables", "render", NULL};

    if (!PyArg_ParseTupleAndKeywords(args, kwargs, "s#|Op", kwlist,
                                     &sql, &sql_len, &tables_list, &render))
        return NULL;

    SyntaqliteValidator *v = syntaqlite_validator_create_sqlite();
    if (!v)
        return PyErr_NoMemory();

    /* Add tables if provided */
    if (tables_list && tables_list != Py_None) {
        if (!PyList_Check(tables_list)) {
            syntaqlite_validator_destroy(v);
            PyErr_SetString(PyExc_TypeError, "tables must be a list");
            return NULL;
        }

        Py_ssize_t n_tables = PyList_Size(tables_list);

        /* Allocate arrays for table definitions */
        SyntaqliteTableDef *table_defs = NULL;
        const char ***all_columns = NULL;

        if (n_tables > 0) {
            table_defs = (SyntaqliteTableDef *)calloc(n_tables, sizeof(SyntaqliteTableDef));
            all_columns = (const char ***)calloc(n_tables, sizeof(const char **));
            if (!table_defs || !all_columns) {
                free(table_defs);
                free(all_columns);
                syntaqlite_validator_destroy(v);
                return PyErr_NoMemory();
            }
        }

        int parse_ok = 1;
        for (Py_ssize_t i = 0; i < n_tables && parse_ok; i++) {
            PyObject *tbl = PyList_GetItem(tables_list, i);
            if (!PyDict_Check(tbl)) {
                PyErr_SetString(PyExc_TypeError, "each table must be a dict with 'name' and optional 'columns'");
                parse_ok = 0;
                break;
            }

            PyObject *name_obj = PyDict_GetItemString(tbl, "name");
            if (!name_obj || !PyUnicode_Check(name_obj)) {
                PyErr_SetString(PyExc_TypeError, "table 'name' must be a string");
                parse_ok = 0;
                break;
            }
            table_defs[i].name = PyUnicode_AsUTF8(name_obj);

            PyObject *cols_obj = PyDict_GetItemString(tbl, "columns");
            if (cols_obj && cols_obj != Py_None && PyList_Check(cols_obj)) {
                Py_ssize_t n_cols = PyList_Size(cols_obj);
                const char **cols = (const char **)calloc(n_cols, sizeof(const char *));
                if (!cols) {
                    parse_ok = 0;
                    PyErr_NoMemory();
                    break;
                }
                for (Py_ssize_t j = 0; j < n_cols; j++) {
                    PyObject *col = PyList_GetItem(cols_obj, j);
                    if (!PyUnicode_Check(col)) {
                        PyErr_SetString(PyExc_TypeError, "column names must be strings");
                        parse_ok = 0;
                        break;
                    }
                    cols[j] = PyUnicode_AsUTF8(col);
                }
                table_defs[i].columns = cols;
                table_defs[i].column_count = (uint32_t)n_cols;
                all_columns[i] = cols;
            } else {
                table_defs[i].columns = NULL;
                table_defs[i].column_count = 0;
                all_columns[i] = NULL;
            }
        }

        if (parse_ok && n_tables > 0) {
            syntaqlite_validator_add_tables(v, table_defs, (uint32_t)n_tables);
        }

        /* Free column arrays */
        if (all_columns) {
            for (Py_ssize_t i = 0; i < n_tables; i++) {
                free((void *)all_columns[i]);
            }
            free(all_columns);
        }
        free(table_defs);

        if (!parse_ok) {
            syntaqlite_validator_destroy(v);
            return NULL;
        }
    }

    uint32_t n_diags = syntaqlite_validator_analyze(v, sql, (uint32_t)sql_len);

    if (render) {
        const char *rendered = syntaqlite_validator_render_diagnostics(v, NULL);
        PyObject *result = PyUnicode_FromString(rendered ? rendered : "");
        syntaqlite_validator_destroy(v);
        return result;
    }

    /* Build list of diagnostic dicts */
    PyObject *result_list = PyList_New(0);
    if (!result_list) {
        syntaqlite_validator_destroy(v);
        return NULL;
    }

    if (n_diags > 0) {
        const SyntaqliteDiagnostic *diags = syntaqlite_validator_diagnostics(v);
        for (uint32_t i = 0; i < n_diags; i++) {
            PyObject *d = PyDict_New();
            if (!d) {
                Py_DECREF(result_list);
                syntaqlite_validator_destroy(v);
                return NULL;
            }

            const char *sev_str;
            switch (diags[i].severity) {
                case SYNTAQLITE_SEVERITY_ERROR:   sev_str = "error"; break;
                case SYNTAQLITE_SEVERITY_WARNING: sev_str = "warning"; break;
                case SYNTAQLITE_SEVERITY_INFO:    sev_str = "info"; break;
                case SYNTAQLITE_SEVERITY_HINT:    sev_str = "hint"; break;
                default:                          sev_str = "unknown"; break;
            }

            PyObject *sev = PyUnicode_FromString(sev_str);
            PyObject *msg = PyUnicode_FromString(diags[i].message ? diags[i].message : "");
            PyObject *start = PyLong_FromUnsignedLong(diags[i].start_offset);
            PyObject *end = PyLong_FromUnsignedLong(diags[i].end_offset);

            if (sev) { PyDict_SetItemString(d, "severity", sev); Py_DECREF(sev); }
            if (msg) { PyDict_SetItemString(d, "message", msg); Py_DECREF(msg); }
            if (start) { PyDict_SetItemString(d, "start_offset", start); Py_DECREF(start); }
            if (end) { PyDict_SetItemString(d, "end_offset", end); Py_DECREF(end); }

            PyList_Append(result_list, d);
            Py_DECREF(d);
        }
    }

    syntaqlite_validator_destroy(v);
    return result_list;
}

/* ─── tokenize ──────────────────────────────────────────────────────── */

static PyObject *
syntaqlite_py_tokenize(PyObject *self, PyObject *args)
{
    const char *sql;
    Py_ssize_t sql_len;

    if (!PyArg_ParseTuple(args, "s#", &sql, &sql_len))
        return NULL;

    PyObject *result_list = PyList_New(0);
    if (!result_list)
        return NULL;

    SyntaqliteTokenizer *tok = syntaqlite_tokenizer_create(NULL);
    if (!tok) {
        Py_DECREF(result_list);
        return PyErr_NoMemory();
    }

    syntaqlite_tokenizer_reset(tok, sql, (uint32_t)sql_len);

    SyntaqliteToken token;
    while (syntaqlite_tokenizer_next(tok, &token)) {
        PyObject *t = PyDict_New();
        if (!t) {
            Py_DECREF(result_list);
            syntaqlite_tokenizer_destroy(tok);
            return NULL;
        }

        PyObject *text = PyUnicode_FromStringAndSize(token.text, token.length);
        PyObject *off = PyLong_FromUnsignedLong((unsigned long)(token.text - sql));
        PyObject *length = PyLong_FromUnsignedLong(token.length);
        PyObject *type = PyLong_FromUnsignedLong(token.type);

        if (text) { PyDict_SetItemString(t, "text", text); Py_DECREF(text); }
        if (off) { PyDict_SetItemString(t, "offset", off); Py_DECREF(off); }
        if (length) { PyDict_SetItemString(t, "length", length); Py_DECREF(length); }
        if (type) { PyDict_SetItemString(t, "type", type); Py_DECREF(type); }

        PyList_Append(result_list, t);
        Py_DECREF(t);
    }

    syntaqlite_tokenizer_destroy(tok);
    return result_list;
}

/* ─── Module definition ─────────────────────────────────────────────── */

static PyMethodDef SyntaqliteMethods[] = {
    {"parse", syntaqlite_py_parse, METH_VARARGS,
     "Parse SQL into a list of statement dicts.\n\n"
     "Each dict has: ok (bool), ast (str or None), error (str or None).\n"
     "For errors: error_offset and error_length are also included."},

    {"format_sql", (PyCFunction)syntaqlite_py_format_sql, METH_VARARGS | METH_KEYWORDS,
     "Format SQL with configurable options.\n\n"
     "Args:\n"
     "    sql (str): SQL to format\n"
     "    line_width (int): Max line width (default 80)\n"
     "    indent_width (int): Spaces per indent (default 2)\n"
     "    keyword_case (str): 'upper' or 'lower' (default 'upper')\n"
     "    semicolons (bool): Append semicolons (default True)\n\n"
     "Returns:\n"
     "    str: Formatted SQL\n\n"
     "Raises:\n"
     "    syntaqlite.FormatError: On parse error"},

    {"validate", (PyCFunction)syntaqlite_py_validate, METH_VARARGS | METH_KEYWORDS,
     "Validate SQL against optional schema.\n\n"
     "Args:\n"
     "    sql (str): SQL to validate\n"
     "    tables (list[dict]): Optional schema. Each dict: name (str), columns (list[str])\n"
     "    render (bool): If True, return rendered diagnostics string instead of list\n\n"
     "Returns:\n"
     "    list[dict] or str: Diagnostics (severity, message, start_offset, end_offset)"},

    {"tokenize", syntaqlite_py_tokenize, METH_VARARGS,
     "Tokenize SQL into a list of token dicts.\n\n"
     "Each dict has: text (str), offset (int), length (int), type (int).\n"
     "All tokens are returned including whitespace and comments."},

    {NULL, NULL, 0, NULL}
};

static struct PyModuleDef syntaqlite_module = {
    PyModuleDef_HEAD_INIT,
    "syntaqlite",
    "Python bindings for syntaqlite — parser, formatter, and validator for SQLite SQL.",
    -1,
    SyntaqliteMethods
};

PyMODINIT_FUNC PyInit_syntaqlite(void) {
    PyObject *m = PyModule_Create(&syntaqlite_module);
    if (m == NULL)
        return NULL;

    FormatError = PyErr_NewException("syntaqlite.FormatError", PyExc_Exception, NULL);
    if (FormatError) {
        Py_INCREF(FormatError);
        PyModule_AddObject(m, "FormatError", FormatError);
    }

    return m;
}
