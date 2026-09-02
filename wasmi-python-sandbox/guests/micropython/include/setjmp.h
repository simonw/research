/*
 * Minimal setjmp.h for building MicroPython with wasi-sdk using LLVM's
 * "emscripten-style" setjmp/longjmp lowering (-mllvm -enable-emscripten-sjlj).
 *
 * wasi-libc refuses to provide setjmp without the Wasm exception-handling
 * proposal, which wasmi 2.0 does not implement. With the emscripten lowering
 * LLVM rewrites every call to setjmp/longjmp into calls to a small runtime
 * (see sjlj_runtime.c) plus `invoke_*` imports that the *host* implements by
 * calling back into the module through its function table.
 */
#ifndef WASMI_SANDBOX_SETJMP_H
#define WASMI_SANDBOX_SETJMP_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* LLVM stores a function-invocation id and a label in the buffer. Keep some
 * slack so the layout is never a problem. */
typedef struct __wasmi_jmp_buf {
    uint32_t func_invocation_id;
    uint32_t label;
    uint32_t reserved[6];
} jmp_buf[1];

int setjmp(jmp_buf env) __attribute__((__returns_twice__));
_Noreturn void longjmp(jmp_buf env, int val);

#define setjmp setjmp

#ifdef __cplusplus
}
#endif

#endif
