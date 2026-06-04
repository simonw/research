#!/usr/bin/env python3
"""
Build script for mquickjs FFI bindings.

This script:
1. Clones mquickjs if not present
2. Builds a shared library with wrapper functions
3. The library provides a simple FFI-friendly API
"""

import os
import subprocess
import sys
import shutil
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
MQUICKJS_DIR = SCRIPT_DIR / "vendor" / "mquickjs"
BUILD_DIR = Path("/tmp/mquickjs-build")


def run(cmd, cwd=None, check=True):
    """Run a command and return output."""
    print(f"  Running: {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if check and result.returncode != 0:
        print(f"STDOUT: {result.stdout}")
        print(f"STDERR: {result.stderr}")
        raise RuntimeError(f"Command failed: {' '.join(cmd)}")
    return result


def ensure_mquickjs():
    """Ensure mquickjs is available in vendor directory."""
    if MQUICKJS_DIR.exists():
        print("mquickjs already available in vendor/")
        return
    # Clone to vendor directory
    print("Cloning mquickjs to vendor/...")
    MQUICKJS_DIR.parent.mkdir(parents=True, exist_ok=True)
    run(["git", "clone", "https://github.com/bellard/mquickjs.git", str(MQUICKJS_DIR)])


def create_wrapper_c():
    """Create a C wrapper with simplified API for FFI."""
    wrapper_c = '''
/* mquickjs sandbox wrapper for FFI */
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <stdint.h>
#include <stdio.h>

#include "cutils.h"
#include "mquickjs.h"

/* These functions are required by the standard library */

/* print() - no-op in sandbox */
static JSValue js_print(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_UNDEFINED;
}

/* gc() - trigger garbage collection */
static JSValue js_gc(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    JS_GC(ctx);
    return JS_UNDEFINED;
}

/* Date.now() */
static JSValue js_date_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    /* Return 0 for determinism in sandbox */
    return JS_NewInt64(ctx, 0);
}

/* performance.now() */
static JSValue js_performance_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    /* Return 0 for determinism in sandbox */
    return JS_NewInt64(ctx, 0);
}

/* load() - disabled in sandbox */
static JSValue js_load(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_ThrowTypeError(ctx, "load() is disabled in sandbox");
}

/* setTimeout - disabled in sandbox */
static JSValue js_setTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_ThrowTypeError(ctx, "setTimeout() is disabled in sandbox");
}

/* clearTimeout - disabled in sandbox */
static JSValue js_clearTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_ThrowTypeError(ctx, "clearTimeout() is disabled in sandbox");
}

/* Include the standard library definition */
#include "mqjs_stdlib.h"

/* Sandbox context */
typedef struct {
    JSContext *ctx;
    uint8_t *mem_buf;
    size_t mem_size;
    int64_t time_limit_ms;
    int64_t start_time_ms;
    int timed_out;
    char *output_buf;
    size_t output_buf_size;
    size_t output_len;
} SandboxContext;

static int64_t get_time_ms(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000 + (ts.tv_nsec / 1000000);
}

/* Interrupt handler for time limit */
static int sandbox_interrupt_handler(JSContext *ctx, void *opaque)
{
    SandboxContext *sctx = (SandboxContext *)opaque;
    if (sctx->time_limit_ms > 0) {
        int64_t elapsed = get_time_ms() - sctx->start_time_ms;
        if (elapsed >= sctx->time_limit_ms) {
            sctx->timed_out = 1;
            return 1; /* interrupt execution */
        }
    }
    return 0;
}

/* Create a new sandbox context */
SandboxContext *sandbox_new(size_t mem_size, int64_t time_limit_ms)
{
    SandboxContext *sctx = malloc(sizeof(SandboxContext));
    if (!sctx)
        return NULL;

    memset(sctx, 0, sizeof(SandboxContext));
    sctx->mem_size = mem_size;
    sctx->time_limit_ms = time_limit_ms;
    sctx->timed_out = 0;

    sctx->mem_buf = malloc(mem_size);
    if (!sctx->mem_buf) {
        free(sctx);
        return NULL;
    }

    sctx->ctx = JS_NewContext(sctx->mem_buf, mem_size, &js_stdlib);
    if (!sctx->ctx) {
        free(sctx->mem_buf);
        free(sctx);
        return NULL;
    }

    JS_SetContextOpaque(sctx->ctx, sctx);
    JS_SetInterruptHandler(sctx->ctx, sandbox_interrupt_handler);
    JS_SetRandomSeed(sctx->ctx, 12345); /* Fixed seed for determinism */

    return sctx;
}

/* Free a sandbox context */
void sandbox_free(SandboxContext *sctx)
{
    if (sctx) {
        if (sctx->ctx)
            JS_FreeContext(sctx->ctx);
        if (sctx->mem_buf)
            free(sctx->mem_buf);
        if (sctx->output_buf)
            free(sctx->output_buf);
        free(sctx);
    }
}

/* Execute JavaScript code and return result as string (JSON-like) */
/* Returns: 0=success, 1=error, 2=timeout, 3=memory error */
int sandbox_eval(SandboxContext *sctx, const char *code, size_t code_len,
                 char *result_buf, size_t result_buf_size,
                 char *error_buf, size_t error_buf_size)
{
    JSValue val;

    if (!sctx || !sctx->ctx) {
        if (error_buf && error_buf_size > 0) {
            strncpy(error_buf, "Invalid context", error_buf_size - 1);
            error_buf[error_buf_size - 1] = 0;
        }
        return 1;
    }

    sctx->start_time_ms = get_time_ms();
    sctx->timed_out = 0;

    val = JS_Eval(sctx->ctx, code, code_len, "<sandbox>", JS_EVAL_RETVAL);

    if (sctx->timed_out) {
        if (error_buf && error_buf_size > 0) {
            strncpy(error_buf, "Execution timeout", error_buf_size - 1);
            error_buf[error_buf_size - 1] = 0;
        }
        return 2;
    }

    if (JS_IsException(val)) {
        if (error_buf && error_buf_size > 0) {
            JS_GetErrorStr(sctx->ctx, error_buf, error_buf_size);
            if (strlen(error_buf) == 0) {
                strncpy(error_buf, "Unknown error", error_buf_size - 1);
                error_buf[error_buf_size - 1] = 0;
            }
        }
        return 1;
    }

    /* Convert result to string */
    if (result_buf && result_buf_size > 0) {
        if (JS_IsUndefined(val)) {
            strncpy(result_buf, "undefined", result_buf_size - 1);
        } else if (JS_IsNull(val)) {
            strncpy(result_buf, "null", result_buf_size - 1);
        } else if (JS_IsBool(val)) {
            strncpy(result_buf, val == JS_TRUE ? "true" : "false", result_buf_size - 1);
        } else if (JS_IsInt(val)) {
            snprintf(result_buf, result_buf_size, "%d", JS_VALUE_GET_INT(val));
        } else if (JS_IsNumber(sctx->ctx, val)) {
            double d;
            JS_ToNumber(sctx->ctx, &d, val);
            snprintf(result_buf, result_buf_size, "%.17g", d);
        } else if (JS_IsString(sctx->ctx, val)) {
            JSCStringBuf buf;
            const char *str = JS_ToCString(sctx->ctx, val, &buf);
            if (str) {
                strncpy(result_buf, str, result_buf_size - 1);
            } else {
                result_buf[0] = 0;
            }
        } else {
            /* For objects/arrays, convert to string representation */
            JSValue str_val = JS_ToString(sctx->ctx, val);
            if (JS_IsString(sctx->ctx, str_val)) {
                JSCStringBuf buf;
                const char *str = JS_ToCString(sctx->ctx, str_val, &buf);
                if (str) {
                    strncpy(result_buf, str, result_buf_size - 1);
                } else {
                    strncpy(result_buf, "[object]", result_buf_size - 1);
                }
            } else {
                strncpy(result_buf, "[object]", result_buf_size - 1);
            }
        }
        result_buf[result_buf_size - 1] = 0;
    }

    return 0;
}

/* Check if timed out */
int sandbox_timed_out(SandboxContext *sctx)
{
    return sctx ? sctx->timed_out : 0;
}
'''
    (BUILD_DIR / "sandbox_wrapper.c").write_text(wrapper_c)


def build_shared_library():
    """Build the shared library."""
    print("Building shared library...")

    # Clean and recreate build directory
    if BUILD_DIR.exists():
        shutil.rmtree(BUILD_DIR)
    BUILD_DIR.mkdir()

    # Copy source files
    for f in ["mquickjs.c", "mquickjs.h", "mquickjs_priv.h", "mquickjs_opcode.h",
              "cutils.c", "cutils.h", "dtoa.c", "dtoa.h", "libm.c", "libm.h",
              "list.h", "mquickjs_build.c", "mquickjs_build.h", "mqjs_stdlib.c",
              "softfp_template.h", "softfp_template_icvt.h"]:
        src = MQUICKJS_DIR / f
        if src.exists():
            shutil.copy(src, BUILD_DIR / f)

    # First build mqjs_stdlib to generate headers
    print("  Building stdlib generator...")
    run(["gcc", "-Wall", "-g", "-O2", "-D_GNU_SOURCE", "-fno-math-errno",
         "-fno-trapping-math", "-c", "-o", "mqjs_stdlib.host.o", "mqjs_stdlib.c"],
        cwd=BUILD_DIR)
    run(["gcc", "-Wall", "-g", "-O2", "-D_GNU_SOURCE", "-fno-math-errno",
         "-fno-trapping-math", "-c", "-o", "mquickjs_build.host.o", "mquickjs_build.c"],
        cwd=BUILD_DIR)
    run(["gcc", "-g", "-o", "mqjs_stdlib_gen", "mqjs_stdlib.host.o", "mquickjs_build.host.o"],
        cwd=BUILD_DIR)

    # Generate headers
    print("  Generating stdlib headers...")
    result = run(["./mqjs_stdlib_gen"], cwd=BUILD_DIR)
    (BUILD_DIR / "mqjs_stdlib.h").write_text(result.stdout)

    result = run(["./mqjs_stdlib_gen", "-a"], cwd=BUILD_DIR)
    (BUILD_DIR / "mquickjs_atom.h").write_text(result.stdout)

    # Create wrapper file
    create_wrapper_c()

    # Build object files with -fPIC for shared library
    print("  Compiling object files...")
    for src, obj in [
        ("mquickjs.c", "mquickjs.o"),
        ("dtoa.c", "dtoa.o"),
        ("libm.c", "libm.o"),
        ("cutils.c", "cutils.o"),
        ("sandbox_wrapper.c", "sandbox_wrapper.o"),
    ]:
        run(["gcc", "-Wall", "-g", "-Os", "-fPIC", "-D_GNU_SOURCE",
             "-fno-math-errno", "-fno-trapping-math",
             "-c", "-o", obj, src], cwd=BUILD_DIR)

    # Link shared library
    print("  Linking shared library...")
    run(["gcc", "-shared", "-o", "libmquickjs_sandbox.so",
         "mquickjs.o", "dtoa.o", "libm.o", "cutils.o", "sandbox_wrapper.o",
         "-lm"], cwd=BUILD_DIR)

    # Copy to script directory
    shutil.copy(BUILD_DIR / "libmquickjs_sandbox.so", SCRIPT_DIR / "libmquickjs_sandbox.so")
    print(f"  Built: {SCRIPT_DIR / 'libmquickjs_sandbox.so'}")


def main():
    print("Building mquickjs FFI bindings...")
    ensure_mquickjs()
    build_shared_library()
    print("Done!")


if __name__ == "__main__":
    main()
