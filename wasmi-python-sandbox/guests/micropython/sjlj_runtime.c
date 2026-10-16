/*
 * Runtime support for LLVM's emscripten-style setjmp/longjmp lowering,
 * written for a plain wasi-sdk build running inside wasmi (which has no
 * Wasm exception handling).
 *
 * How it works:
 *  - In any function that calls setjmp, LLVM rewrites every call that could
 *    longjmp into a call of an imported `invoke_<sig>(fnptr, args...)`.
 *  - The host implements `invoke_*` by calling the function-table entry and
 *    catching a "longjmp unwind" error coming out of it.
 *  - longjmp becomes `emscripten_longjmp(env, val)` (below), which records
 *    the target and calls the imported `_emscripten_throw_longjmp`, which the
 *    host turns into an error that unwinds the nested call.
 *  - Back in the setjmp-containing function, generated code inspects
 *    `__THREW__`/`__threwValue` and `__wasm_setjmp_test` to decide whether
 *    this frame owns the jmp_buf (then it "returns" from setjmp with the value)
 *    or must re-throw.
 */
#include <stdint.h>

struct jmp_buf_impl {
    uint32_t func_invocation_id;
    uint32_t label;
};

/* Globals consulted by LLVM-generated code after each invoke. */
uintptr_t __THREW__ = 0;
int __threwValue = 0;

__attribute__((import_module("env"), import_name("_emscripten_throw_longjmp")))
void _emscripten_throw_longjmp(void);

void __wasm_setjmp(void *env, uint32_t label, void *func_invocation_id) {
    struct jmp_buf_impl *jb = env;
    jb->func_invocation_id = (uint32_t)(uintptr_t)func_invocation_id;
    jb->label = label;
}

uint32_t __wasm_setjmp_test(void *env, void *func_invocation_id) {
    struct jmp_buf_impl *jb = env;
    if (jb->label != 0 && jb->func_invocation_id == (uint32_t)(uintptr_t)func_invocation_id) {
        return jb->label;
    }
    return 0;
}

_Noreturn void emscripten_longjmp(uintptr_t env, int val) {
    __THREW__ = env;
    __threwValue = val;
    _emscripten_throw_longjmp();
    __builtin_unreachable();
}

static uint32_t temp_ret0;
void setTempRet0(uint32_t v) { temp_ret0 = v; }
uint32_t getTempRet0(void) { return temp_ret0; }

/* Called by the host's invoke_* implementation after catching an unwind. */
__attribute__((export_name("setThrew")))
void setThrew(uintptr_t threw, int value) {
    if (__THREW__ == 0) {
        __THREW__ = threw;
        __threwValue = value;
    }
}

/* Shadow stack pointer save/restore, used by the host around invoke_* calls. */
__attribute__((export_name("stack_save")))
uintptr_t stack_save(void) {
    uintptr_t sp;
    __asm__ volatile(".globaltype __stack_pointer, i32\nglobal.get __stack_pointer\nlocal.set %0" : "=r"(sp));
    return sp;
}

__attribute__((export_name("stack_restore")))
void stack_restore(uintptr_t sp) {
    __asm__ volatile(".globaltype __stack_pointer, i32\nlocal.get %0\nglobal.set __stack_pointer" : : "r"(sp));
}
