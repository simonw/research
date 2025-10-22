/*
 * Python C extension module for cmarkgfm without CFFI
 * This is a Pyodide-compatible implementation using Python's C API
 */

#define PY_SSIZE_T_CLEAN
#include <Python.h>
#include "cmark-gfm.h"
#include "cmark-gfm-extension_api.h"
#include "cmark-gfm-core-extensions.h"

/* Forward declarations */
static PyObject* cmarkgfm_markdown_to_html(PyObject* self, PyObject* args, PyObject* kwargs);
static PyObject* cmarkgfm_github_flavored_markdown_to_html(PyObject* self, PyObject* args, PyObject* kwargs);

/* Module methods */
static PyMethodDef CmarkgfmMethods[] = {
    {"markdown_to_html", (PyCFunction)cmarkgfm_markdown_to_html,
     METH_VARARGS | METH_KEYWORDS,
     "Convert Markdown to HTML\n\n"
     "Args:\n"
     "    text (str): Markdown text to convert\n"
     "    options (int): Rendering options (default: 0)\n\n"
     "Returns:\n"
     "    str: HTML output"},

    {"github_flavored_markdown_to_html", (PyCFunction)cmarkgfm_github_flavored_markdown_to_html,
     METH_VARARGS | METH_KEYWORDS,
     "Convert GitHub Flavored Markdown to HTML\n\n"
     "Args:\n"
     "    text (str): Markdown text to convert\n"
     "    options (int): Additional rendering options (default: 0)\n\n"
     "Returns:\n"
     "    str: HTML output"},

    {NULL, NULL, 0, NULL}  /* Sentinel */
};

/* Module definition */
static struct PyModuleDef cmarkgfm_module = {
    PyModuleDef_HEAD_INIT,
    "_cmarkgfm",     /* Module name */
    "Python bindings for cmark-gfm (GitHub Flavored Markdown) without CFFI",  /* Module docstring */
    -1,             /* Size of per-interpreter state, -1 means module keeps state in global variables */
    CmarkgfmMethods
};

/* markdown_to_html implementation */
static PyObject* cmarkgfm_markdown_to_html(PyObject* self, PyObject* args, PyObject* kwargs) {
    const char* markdown_text;
    Py_ssize_t text_len;
    int options = 0;

    static char* kwlist[] = {"text", "options", NULL};

    if (!PyArg_ParseTupleAndKeywords(args, kwargs, "s#|i", kwlist,
                                      &markdown_text, &text_len, &options)) {
        return NULL;
    }

    /* Convert markdown to HTML using cmark */
    char* html = cmark_markdown_to_html(markdown_text, text_len, options);

    if (html == NULL) {
        PyErr_SetString(PyExc_RuntimeError, "Failed to convert markdown to HTML");
        return NULL;
    }

    /* Create Python string from C string */
    PyObject* result = PyUnicode_FromString(html);

    /* Free the C string allocated by cmark */
    free(html);

    return result;
}

/* github_flavored_markdown_to_html implementation */
static PyObject* cmarkgfm_github_flavored_markdown_to_html(PyObject* self, PyObject* args, PyObject* kwargs) {
    const char* markdown_text;
    Py_ssize_t text_len;
    int options = 0;

    static char* kwlist[] = {"text", "options", NULL};

    if (!PyArg_ParseTupleAndKeywords(args, kwargs, "s#|i", kwlist,
                                      &markdown_text, &text_len, &options)) {
        return NULL;
    }

    /* Ensure GFM extensions are registered */
    cmark_gfm_core_extensions_ensure_registered();

    /* Add GitHub-specific options */
    options |= CMARK_OPT_GITHUB_PRE_LANG;

    /* Create parser */
    cmark_parser* parser = cmark_parser_new(options);
    if (parser == NULL) {
        PyErr_SetString(PyExc_RuntimeError, "Failed to create parser");
        return NULL;
    }

    /* Enable GFM extensions */
    const char* extensions[] = {"table", "strikethrough", "autolink", "tagfilter", "tasklist"};
    for (int i = 0; i < 5; i++) {
        cmark_syntax_extension* ext = cmark_find_syntax_extension(extensions[i]);
        if (ext) {
            cmark_parser_attach_syntax_extension(parser, ext);
        }
    }

    /* Parse the markdown */
    cmark_parser_feed(parser, markdown_text, text_len);
    cmark_node* document = cmark_parser_finish(parser);

    if (document == NULL) {
        cmark_parser_free(parser);
        PyErr_SetString(PyExc_RuntimeError, "Failed to parse markdown");
        return NULL;
    }

    /* Render to HTML */
    cmark_llist* ext_list = cmark_parser_get_syntax_extensions(parser);
    char* html = cmark_render_html(document, options, ext_list);

    /* Clean up */
    cmark_node_free(document);
    cmark_parser_free(parser);

    if (html == NULL) {
        PyErr_SetString(PyExc_RuntimeError, "Failed to render HTML");
        return NULL;
    }

    /* Create Python string */
    PyObject* result = PyUnicode_FromString(html);
    free(html);

    return result;
}

/* Module initialization */
PyMODINIT_FUNC PyInit__cmarkgfm(void) {
    PyObject* module = PyModule_Create(&cmarkgfm_module);
    if (module == NULL) {
        return NULL;
    }

    /* Add version constant */
    PyModule_AddStringConstant(module, "__version__", "2025.10.22.pyodide");
    PyModule_AddStringConstant(module, "CMARK_VERSION", "0.29.0.gfm.2");

    /* Add option constants */
    PyModule_AddIntConstant(module, "CMARK_OPT_DEFAULT", CMARK_OPT_DEFAULT);
    PyModule_AddIntConstant(module, "CMARK_OPT_SOURCEPOS", CMARK_OPT_SOURCEPOS);
    PyModule_AddIntConstant(module, "CMARK_OPT_HARDBREAKS", CMARK_OPT_HARDBREAKS);
    PyModule_AddIntConstant(module, "CMARK_OPT_UNSAFE", CMARK_OPT_UNSAFE);
    PyModule_AddIntConstant(module, "CMARK_OPT_NOBREAKS", CMARK_OPT_NOBREAKS);
    PyModule_AddIntConstant(module, "CMARK_OPT_NORMALIZE", CMARK_OPT_NORMALIZE);
    PyModule_AddIntConstant(module, "CMARK_OPT_VALIDATE_UTF8", CMARK_OPT_VALIDATE_UTF8);
    PyModule_AddIntConstant(module, "CMARK_OPT_SMART", CMARK_OPT_SMART);
    PyModule_AddIntConstant(module, "CMARK_OPT_GITHUB_PRE_LANG", CMARK_OPT_GITHUB_PRE_LANG);
    PyModule_AddIntConstant(module, "CMARK_OPT_LIBERAL_HTML_TAG", CMARK_OPT_LIBERAL_HTML_TAG);
    PyModule_AddIntConstant(module, "CMARK_OPT_FOOTNOTES", CMARK_OPT_FOOTNOTES);
    PyModule_AddIntConstant(module, "CMARK_OPT_STRIKETHROUGH_DOUBLE_TILDE",
                            CMARK_OPT_STRIKETHROUGH_DOUBLE_TILDE);
    PyModule_AddIntConstant(module, "CMARK_OPT_TABLE_PREFER_STYLE_ATTRIBUTES",
                            CMARK_OPT_TABLE_PREFER_STYLE_ATTRIBUTES);

    return module;
}
