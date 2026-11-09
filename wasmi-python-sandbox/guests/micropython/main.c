// Entry points exported by the MicroPython wasm guest.
//
// Exports:
//   mp_sandbox_init(heap_bytes)      initialise the interpreter
//   mp_sandbox_exec(src_ptr, len)    compile+run Python source, returns 0 ok / 1 exception
//   mp_sandbox_deinit()
//   malloc/free (from libc)          used by the host to place strings in memory
// Imports (module "env"):
//   host_write(fd, ptr, len)         stdout=1 stderr=2
//   host_call(...) / host_take(...)  see modhost.c
//   invoke_* / _emscripten_throw_longjmp   see sjlj_runtime.c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "py/compile.h"
#include "py/gc.h"
#include "py/runtime.h"
#include "py/stackctrl.h"
#include "py/mphal.h"
#include "py/builtin.h"
#include "py/mperrno.h"

__attribute__((import_module("env"), import_name("host_write")))
void host_write(int fd, const char *ptr, size_t len);

// Linker-provided: the wasm shadow stack size is set with -z stack-size.
extern unsigned char __stack_low, __stack_high;

static bool gc_collect_pending = false;

// Print to stderr for uncaught exceptions.
static void stderr_print_strn(void *env, const char *str, size_t len) {
    (void)env;
    host_write(2, str, len);
}
const mp_print_t mp_stderr_print = {NULL, stderr_print_strn};

__attribute__((export_name("mp_sandbox_init")))
int mp_sandbox_init(size_t heap_bytes) {
    // The shadow stack grows down from __stack_high; leave a margin.
    mp_stack_set_top(&__stack_high);
    mp_stack_set_limit((size_t)(&__stack_high - &__stack_low) - MICROPY_STACK_CHECK_MARGIN);

    char *heap = malloc(heap_bytes);
    if (heap == NULL) {
        return -1;
    }
    gc_init(heap, heap + heap_bytes);
    // Trigger (deferred) collections early so the auto-split heap does not
    // grow without bound between top-level calls.
    MP_STATE_MEM(gc_alloc_threshold) = 64 * 1024 / MICROPY_BYTES_PER_GC_BLOCK;
    mp_init();
    return 0;
}

// Collect at the top level, where there are no MicroPython pointers hiding in
// Wasm locals (the conservative scanner can only see linear memory).
static void gc_collect_top_level(void) {
    if (gc_collect_pending) {
        gc_collect_pending = false;
        gc_collect_start();
        gc_collect_end();
    }
}

__attribute__((export_name("mp_sandbox_exec")))
int mp_sandbox_exec(const char *src, size_t len) {
    int status = 0;
    nlr_buf_t nlr;
    if (nlr_push(&nlr) == 0) {
        mp_lexer_t *lex = mp_lexer_new_from_str_len(MP_QSTR__lt_stdin_gt_, src, len, 0);
        qstr source_name = lex->source_name;
        mp_parse_tree_t parse_tree = mp_parse(lex, MP_PARSE_FILE_INPUT);
        mp_obj_t module_fun = mp_compile(&parse_tree, source_name, false);
        mp_call_function_0(module_fun);
        nlr_pop();
    } else {
        mp_obj_print_exception(&mp_stderr_print, (mp_obj_t)nlr.ret_val);
        status = 1;
    }
    gc_collect_top_level();
    return status;
}

// Called by the host (instead of the function it was asked to invoke) when
// its own native stack is nearly exhausted by nested guest calls.
__attribute__((export_name("mp_sandbox_recursion_error")))
void mp_sandbox_recursion_error(void) {
    mp_raise_recursion_depth();
}

__attribute__((export_name("mp_sandbox_collect")))
void mp_sandbox_collect(void) {
    gc_collect_pending = true;
    gc_collect_top_level();
}

__attribute__((export_name("mp_sandbox_deinit")))
void mp_sandbox_deinit(void) {
    mp_deinit();
}

// ---- GC hooks -------------------------------------------------------------

void gc_collect(void) {
    gc_collect_pending = true;
}

#if MICROPY_GC_SPLIT_HEAP_AUTO
// How large a new heap area may be. The real bound is the wasm memory limit
// enforced by the host: when memory.grow fails, malloc returns NULL and the
// GC raises MemoryError inside the guest.
size_t gc_get_max_new_split(void) {
    return 64 * 1024 * 1024;
}
#endif

// ---- misc port hooks ------------------------------------------------------

mp_lexer_t *mp_lexer_new_from_file(qstr filename) {
    mp_raise_OSError(MP_ENOENT);
}

mp_import_stat_t mp_import_stat(const char *path) {
    return MP_IMPORT_STAT_NO_EXIST;
}

mp_obj_t mp_builtin_open(size_t n_args, const mp_obj_t *args, mp_map_t *kwargs) {
    mp_raise_OSError(MP_EPERM);
}
MP_DEFINE_CONST_FUN_OBJ_KW(mp_builtin_open_obj, 1, mp_builtin_open);

void nlr_jump_fail(void *val) {
    static const char msg[] = "FATAL: uncaught NLR\n";
    host_write(2, msg, sizeof(msg) - 1);
    abort();
}

#ifndef NDEBUG
void __assert_func(const char *file, int line, const char *func, const char *expr) {
    static const char msg[] = "assertion failed\n";
    host_write(2, msg, sizeof(msg) - 1);
    abort();
}
#endif
