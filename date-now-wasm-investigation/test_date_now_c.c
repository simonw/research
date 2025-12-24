/*
 * Test Date.now() in original mquickjs C implementation
 */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <sys/time.h>
#include <stdint.h>

#include "cutils.h"
#include "mquickjs.h"

/* Required functions for stdlib */
static JSValue js_print(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_UNDEFINED;
}

static JSValue js_gc(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    JS_GC(ctx);
    return JS_UNDEFINED;
}

/* Date.now() - returns actual time (like the original mqjs.c) */
static JSValue js_date_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return JS_NewInt64(ctx, (int64_t)tv.tv_sec * 1000 + (tv.tv_usec / 1000));
}

/* performance.now() */
static JSValue js_performance_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return JS_NewInt64(ctx, (int64_t)ts.tv_sec * 1000 + (ts.tv_nsec / 1000000));
}

static JSValue js_load(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_ThrowTypeError(ctx, "load() disabled");
}

static JSValue js_setTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_ThrowTypeError(ctx, "setTimeout() disabled");
}

static JSValue js_clearTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_ThrowTypeError(ctx, "clearTimeout() disabled");
}

#include "mqjs_stdlib.h"

int main()
{
    uint8_t *mem_buf;
    JSContext *ctx;
    JSValue val;
    char result_buf[256];
    size_t mem_size = 1024 * 1024;  /* 1MB */

    printf("Testing Date.now() in C with REAL time implementation\n\n");

    mem_buf = malloc(mem_size);
    if (!mem_buf) {
        fprintf(stderr, "Failed to allocate memory\n");
        return 1;
    }

    ctx = JS_NewContext(mem_buf, mem_size, &js_stdlib);
    if (!ctx) {
        fprintf(stderr, "Failed to create JS context\n");
        free(mem_buf);
        return 1;
    }

    /* Test Date.now() */
    printf("Testing Date.now():\n");
    for (int i = 0; i < 3; i++) {
        val = JS_Eval(ctx, "Date.now()", 10, "<test>", JS_EVAL_RETVAL);
        if (JS_IsException(val)) {
            JS_GetErrorStr(ctx, result_buf, sizeof(result_buf));
            printf("  Error: %s\n", result_buf);
        } else if (JS_IsInt(val)) {
            printf("  Call %d: %d\n", i + 1, JS_VALUE_GET_INT(val));
        } else if (JS_IsNumber(ctx, val)) {
            double d;
            JS_ToNumber(ctx, &d, val);
            printf("  Call %d: %.0f\n", i + 1, d);
        }
        /* Small delay to see time difference */
        struct timespec ts = {0, 10000000}; /* 10ms */
        nanosleep(&ts, NULL);
    }

    /* Test performance.now() */
    printf("\nTesting performance.now():\n");
    val = JS_Eval(ctx, "performance.now()", 17, "<test>", JS_EVAL_RETVAL);
    if (JS_IsNumber(ctx, val)) {
        double d;
        JS_ToNumber(ctx, &d, val);
        printf("  Result: %.0f\n", d);
    }

    /* Compare with C time */
    printf("\nNative C time for comparison:\n");
    struct timeval tv;
    gettimeofday(&tv, NULL);
    printf("  %ld\n", (long)((int64_t)tv.tv_sec * 1000 + (tv.tv_usec / 1000)));

    JS_FreeContext(ctx);
    free(mem_buf);

    return 0;
}
