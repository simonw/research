// MicroPython configuration for the wasmi sandbox guest.
#include <port/mpconfigport_common.h>
#include <stdlib.h> // malloc/free for MICROPY_GC_SPLIT_HEAP_AUTO

// A reasonably featured interpreter: floats, complex, lots of builtins.
#define MICROPY_CONFIG_ROM_LEVEL                (MICROPY_CONFIG_ROM_LEVEL_CORE_FEATURES)

#define MICROPY_ENABLE_COMPILER                 (1)
#define MICROPY_ENABLE_GC                       (1)
#define MICROPY_PY_GC                           (1)
#define MICROPY_FLOAT_IMPL                      (MICROPY_FLOAT_IMPL_DOUBLE)
#define MICROPY_LONGINT_IMPL                    (MICROPY_LONGINT_IMPL_MPZ)
#define MICROPY_ENABLE_DOC_STRING               (0)
#define MICROPY_ERROR_REPORTING                 (MICROPY_ERROR_REPORTING_DETAILED)
#define MICROPY_WARNINGS                        (0)
#define MICROPY_PY_BUILTINS_HELP                (0)
#define MICROPY_PY_SYS                          (0)
#define MICROPY_PY_JSON                         (1)
#define MICROPY_PY_IO                           (1)
#define MICROPY_KBD_EXCEPTION                   (0)
#define MICROPY_USE_INTERNAL_PRINTF             (0)
#define MICROPY_USE_INTERNAL_ERRNO              (1)

// Convert C-stack exhaustion into a Python RuntimeError instead of a wasm trap.
#define MICROPY_STACK_CHECK                     (1)
#define MICROPY_STACK_CHECK_MARGIN              (32 * 1024)

// Like the official webassembly port: the GC heap grows on demand (bounded by
// the wasm memory limit set by the host) and actual collections are deferred
// to the top level, where no MicroPython pointers live in Wasm locals that a
// conservative stack scan could miss.
#define MICROPY_GC_SPLIT_HEAP                   (1)
#define MICROPY_GC_SPLIT_HEAP_AUTO              (1)
#define MICROPY_GC_ALLOC_THRESHOLD              (1)

#define MICROPY_PY_SYS_PLATFORM                 "wasmi-sandbox"
#define MICROPY_HW_BOARD_NAME                   "wasmi"
#define MICROPY_HW_MCU_NAME                     "wasm32"

#define MP_STATE_PORT MP_STATE_VM
