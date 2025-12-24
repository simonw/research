/*
 * Test Date.now() with STUBBED implementation (sandbox version)
 */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>

#include "cutils.h"
#include "mquickjs.h"

/* Required functions for stdlib */
static JSValue js_print(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_UNDEFINED; }
static JSValue js_gc(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { JS_GC(ctx); return JS_UNDEFINED; }

/* Date.now() - STUBBED to return 0 (sandbox version) */
static JSValue js_date_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_NewInt64(ctx, 0);  /* Always returns 0 for determinism */
}

/* performance.now() - STUBBED */
static JSValue js_performance_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_NewInt64(ctx, 0);  /* Always returns 0 for determinism */
}

static JSValue js_load(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_ThrowTypeError(ctx, "disabled"); }
static JSValue js_setTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_ThrowTypeError(ctx, "disabled"); }
static JSValue js_clearTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_ThrowTypeError(ctx, "disabled"); }

#include "mqjs_stdlib.h"

int main()
{
    uint8_t *mem_buf;
    JSContext *ctx;
    JSValue val;
    size_t mem_size = 1024 * 1024;

    printf("Testing Date.now() in C with STUBBED implementation (sandbox)\n\n");

    mem_buf = malloc(mem_size);
    ctx = JS_NewContext(mem_buf, mem_size, &js_stdlib);

    printf("Testing Date.now():\n");
    for (int i = 0; i < 3; i++) {
        val = JS_Eval(ctx, "Date.now()", 10, "<test>", JS_EVAL_RETVAL);
        if (JS_IsNumber(ctx, val)) {
            double d;
            JS_ToNumber(ctx, &d, val);
            printf("  Call %d: %.0f\n", i + 1, d);
        }
    }

    printf("\nTesting performance.now():\n");
    val = JS_Eval(ctx, "performance.now()", 17, "<test>", JS_EVAL_RETVAL);
    if (JS_IsNumber(ctx, val)) {
        double d;
        JS_ToNumber(ctx, &d, val);
        printf("  Result: %.0f\n", d);
    }

    JS_FreeContext(ctx);
    free(mem_buf);

    return 0;
}
