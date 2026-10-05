/*
 * QuickJS (quickjs-ng) reactor for the wasmi sandbox.
 *
 * Exports:
 *   qjs_init(memory_limit, stack_size)   create runtime + context (0 on success)
 *   qjs_eval(code_ptr, code_len)          run JS; 0 = ok (JSON result on fd 3),
 *                                         1 = exception (text on fd 4)
 *   qjs_gc()                              run the JS garbage collector
 *   qjs_memory_usage()                    bytes currently allocated by QuickJS
 *   qjs_destroy()
 *   malloc/free                           for the host to place strings in memory
 *
 * Imports (module "env"), all implemented in Python:
 *   host_write(fd, ptr, len)              1=stdout 2=stderr 3=result 4=exception
 *   host_call(name, name_len, json, len)  call a host function; returns result length
 *                                         (negative = error message length)
 *   host_take(buf, len)                   copy the pending host result into buf
 *   host_interrupt()                      non-zero to interrupt running JS (soft limit)
 *
 * JS globals provided: print(...), console.log/error(...), host.<name>(...args)
 */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "quickjs.h"

__attribute__((import_module("env"), import_name("host_write")))
void host_write(int fd, const char *ptr, size_t len);

__attribute__((import_module("env"), import_name("host_call")))
int host_call(const char *name, size_t name_len, const char *json, size_t json_len);

__attribute__((import_module("env"), import_name("host_take")))
void host_take(char *buf, size_t len);

__attribute__((import_module("env"), import_name("host_interrupt")))
int host_interrupt(void);

static JSRuntime *rt;
static JSContext *ctx;

static void write_value(int fd, JSContext *c, JSValueConst v) {
    size_t len;
    const char *s = JS_ToCStringLen(c, &len, v);
    if (s) {
        host_write(fd, s, len);
        JS_FreeCString(c, s);
    } else {
        host_write(fd, "[unprintable]", 13);
    }
}

static JSValue js_print_to(JSContext *c, int fd, int argc, JSValueConst *argv) {
    for (int i = 0; i < argc; i++) {
        if (i) {
            host_write(fd, " ", 1);
        }
        write_value(fd, c, argv[i]);
    }
    host_write(fd, "\n", 1);
    return JS_UNDEFINED;
}

static JSValue js_print(JSContext *c, JSValueConst this_val, int argc, JSValueConst *argv) {
    return js_print_to(c, 1, argc, argv);
}

static JSValue js_print_err(JSContext *c, JSValueConst this_val, int argc, JSValueConst *argv) {
    return js_print_to(c, 2, argc, argv);
}

/* __host_call(name, jsonArgs) -> parsed JSON result, or throws */
static JSValue js_host_call(JSContext *c, JSValueConst this_val, int argc, JSValueConst *argv) {
    if (argc < 2) {
        return JS_ThrowTypeError(c, "__host_call(name, json) requires 2 arguments");
    }
    size_t name_len, json_len;
    const char *name = JS_ToCStringLen(c, &name_len, argv[0]);
    if (!name) {
        return JS_EXCEPTION;
    }
    const char *json = JS_ToCStringLen(c, &json_len, argv[1]);
    if (!json) {
        JS_FreeCString(c, name);
        return JS_EXCEPTION;
    }
    int ret = host_call(name, name_len, json, json_len);
    JS_FreeCString(c, name);
    JS_FreeCString(c, json);

    size_t out_len = (size_t)(ret < 0 ? -ret : ret);
    char *buf = js_malloc(c, out_len + 1);
    if (!buf) {
        return JS_EXCEPTION;
    }
    host_take(buf, out_len);
    buf[out_len] = '\0';
    JSValue result;
    if (ret < 0) {
        result = JS_ThrowPlainError(c, "%s", buf);
    } else {
        result = JS_ParseJSON(c, buf, out_len, "<host>");
    }
    js_free(c, buf);
    return result;
}

static int interrupt_handler(JSRuntime *r, void *opaque) {
    return host_interrupt();
}

static const char bootstrap_js[] =
    "globalThis.console = { log: print, info: print, warn: __print_err, error: __print_err };\n"
    "globalThis.host = new Proxy({}, {\n"
    "  get(_, name) { return (...args) => __host_call(String(name), JSON.stringify(args)); }\n"
    "});\n";

__attribute__((export_name("qjs_init")))
int qjs_init(size_t memory_limit, size_t stack_size) {
    if (rt) {
        return -1;
    }
    rt = JS_NewRuntime();
    if (!rt) {
        return -2;
    }
    if (memory_limit) {
        JS_SetMemoryLimit(rt, memory_limit);
    }
    if (stack_size) {
        JS_SetMaxStackSize(rt, stack_size);
    }
    JS_SetInterruptHandler(rt, interrupt_handler, NULL);
    ctx = JS_NewContext(rt);
    if (!ctx) {
        return -3;
    }
    JSValue global = JS_GetGlobalObject(ctx);
    JS_SetPropertyStr(ctx, global, "print", JS_NewCFunction(ctx, js_print, "print", 1));
    JS_SetPropertyStr(ctx, global, "__print_err", JS_NewCFunction(ctx, js_print_err, "__print_err", 1));
    JS_SetPropertyStr(ctx, global, "__host_call", JS_NewCFunction(ctx, js_host_call, "__host_call", 2));
    JS_FreeValue(ctx, global);
    JSValue r = JS_Eval(ctx, bootstrap_js, sizeof(bootstrap_js) - 1, "<bootstrap>", JS_EVAL_TYPE_GLOBAL);
    int status = JS_IsException(r) ? -4 : 0;
    JS_FreeValue(ctx, r);
    return status;
}

static void report_exception(JSContext *c) {
    JSValue exc = JS_GetException(c);
    if (JS_IsError(exc)) {
        JSValue stack = JS_GetPropertyStr(c, exc, "stack");
        write_value(4, c, exc);
        if (!JS_IsUndefined(stack)) {
            host_write(4, "\n", 1);
            write_value(4, c, stack);
        }
        JS_FreeValue(c, stack);
    } else {
        write_value(4, c, exc);
    }
    JS_FreeValue(c, exc);
}

__attribute__((export_name("qjs_eval")))
int qjs_eval(const char *code, size_t len) {
    if (!ctx) {
        return -1;
    }
    JSValue r = JS_Eval(ctx, code, len, "<eval>", JS_EVAL_TYPE_GLOBAL);
    if (JS_IsException(r)) {
        report_exception(ctx);
        JS_FreeValue(ctx, r);
        return 1;
    }
    /* Drain promise jobs. */
    for (;;) {
        JSContext *jctx;
        int n = JS_ExecutePendingJob(rt, &jctx);
        if (n <= 0) {
            if (n < 0) {
                report_exception(jctx);
                JS_FreeValue(ctx, r);
                return 1;
            }
            break;
        }
    }
    /* Report the completion value as JSON (falling back to String()). */
    JSValue json = JS_JSONStringify(ctx, r, JS_UNDEFINED, JS_UNDEFINED);
    if (JS_IsException(json)) {
        report_exception(ctx);
        JS_FreeValue(ctx, r);
        return 1;
    }
    if (JS_IsUndefined(json)) {
        write_value(3, ctx, r);
    } else {
        write_value(3, ctx, json);
    }
    JS_FreeValue(ctx, json);
    JS_FreeValue(ctx, r);
    return 0;
}

__attribute__((export_name("qjs_gc")))
void qjs_gc(void) {
    if (rt) {
        JS_RunGC(rt);
    }
}

__attribute__((export_name("qjs_memory_usage")))
int64_t qjs_memory_usage(void) {
    if (!rt) {
        return 0;
    }
    JSMemoryUsage u;
    JS_ComputeMemoryUsage(rt, &u);
    return u.malloc_size;
}

__attribute__((export_name("qjs_destroy")))
void qjs_destroy(void) {
    if (ctx) {
        JS_FreeContext(ctx);
        ctx = NULL;
    }
    if (rt) {
        JS_FreeRuntime(rt);
        rt = NULL;
    }
}
