"""
Luau sandbox using wasmtime.

Loads the Luau WASM module (compiled via emscripten) and provides a Python
interface. Implements the emscripten runtime imports including C++ exception
handling (invoke_* trampolines and __cxa_* functions) needed by Luau.

Based on the approach from mquickjs-sandbox/mquickjs_wasmtime.py.
"""

import struct
import time
from pathlib import Path

import wasmtime

SCRIPT_DIR = Path(__file__).parent
WASM_PATH = SCRIPT_DIR / "luau.wasm"


class LongjmpException(Exception):
    """Raised when longjmp or __cxa_throw is called in WASM."""
    pass


class LuauWasmtime:
    """
    Luau sandbox using wasmtime with full C++ exception support.

    The emscripten-compiled Luau WASM requires:
    - invoke_* trampolines for indirect function calls with exception catching
    - __cxa_* functions for C++ exception handling
    - WASI imports for clock_time_get and fd_write
    - Various emscripten runtime stubs
    """

    def __init__(self, wasm_path=None):
        wasm_path = Path(wasm_path or WASM_PATH)

        # Track C++ exception state
        self._exception_last = 0
        self._uncaught_exception_count = 0
        self._exception_caught = []
        self._temp_ret0 = 0
        self._output_buffer = []

        # Create wasmtime engine and store
        self.engine = wasmtime.Engine()
        self.store = wasmtime.Store(self.engine)

        # Load WASM module
        wasm_bytes = wasm_path.read_bytes()
        self.module = wasmtime.Module(self.engine, wasm_bytes)

        # Create linker
        self.linker = wasmtime.Linker(self.engine)

        # Add WASI (for clock_time_get and fd_write)
        wasi_config = wasmtime.WasiConfig()
        # Don't inherit stdout - we capture it
        self.store.set_wasi(wasi_config)
        self.linker.define_wasi()

        # Add emscripten env imports
        self._add_env_imports()

        # Instantiate
        self.instance = self.linker.instantiate(self.store, self.module)

        # Get exports
        exports = self.instance.exports(self.store)
        self._memory = exports["memory"]
        self._executeScript = exports["executeScript"]
        self._table = exports["__indirect_function_table"]
        self._setThrew = exports["setThrew"]
        self._emscripten_tempret_set = exports["_emscripten_tempret_set"]
        self._stackRestore = exports["_emscripten_stack_restore"]
        self._stackSave = exports["emscripten_stack_get_current"]
        self._stackAlloc = exports["_emscripten_stack_alloc"]
        self._cxa_increment_refcount = exports["__cxa_increment_exception_refcount"]
        self._cxa_decrement_refcount = exports["__cxa_decrement_exception_refcount"]
        self._cxa_can_catch = exports["__cxa_can_catch"]
        self._cxa_get_exception_ptr = exports["__cxa_get_exception_ptr"]
        self._cxa_free_exception = exports["__cxa_free_exception"]

        # Call ctors to initialize C++ runtime
        wasm_call_ctors = exports["__wasm_call_ctors"]
        wasm_call_ctors(self.store)

        self._initialized = True

    def _add_env_imports(self):
        """Register all emscripten 'env' imports."""
        i32 = wasmtime.ValType.i32()
        i64 = wasmtime.ValType.i64()
        f64 = wasmtime.ValType.f64()

        # ---- invoke_* trampolines ----
        # Dynamically discover and register all invoke_* imports
        for imp in self.module.imports:
            if not imp.name.startswith("invoke_"):
                continue

            name = imp.name
            func_type = imp.type
            has_return = len(list(func_type.results)) > 0

            def make_invoke(has_ret):
                def invoke_fn(*args):
                    index = args[0]
                    params = args[1:]
                    stack_ptr = self._stackSave(self.store)
                    try:
                        func = self._table.get(self.store, index)
                        if func is None:
                            return 0 if has_ret else None
                        result = func(self.store, *params)
                        return result
                    except (LongjmpException, wasmtime.Trap, Exception) as e:
                        self._setThrew(self.store, 1, 0)
                        self._stackRestore(self.store, stack_ptr)
                        return 0 if has_ret else None
                return invoke_fn

            fn = wasmtime.Func(self.store, func_type, make_invoke(has_return))
            self.linker.define(self.store, "env", name, fn)

        # ---- C++ exception handling ----

        # __cxa_throw(ptr, type, destructor)
        def cxa_throw(ptr, type_info, destructor):
            self._exception_last = ptr
            self._uncaught_exception_count += 1
            raise LongjmpException(f"__cxa_throw({ptr})")

        self.linker.define(self.store, "env", "__cxa_throw",
            wasmtime.Func(self.store, wasmtime.FuncType([i32, i32, i32], []), cxa_throw))

        # __cxa_begin_catch(ptr) -> ptr
        def cxa_begin_catch(ptr):
            self._exception_caught.append(ptr)
            if self._uncaught_exception_count > 0:
                self._uncaught_exception_count -= 1
            return ptr

        self.linker.define(self.store, "env", "__cxa_begin_catch",
            wasmtime.Func(self.store, wasmtime.FuncType([i32], [i32]), cxa_begin_catch))

        # __cxa_end_catch()
        def cxa_end_catch():
            if self._exception_caught:
                info = self._exception_caught.pop()
            self._exception_last = 0

        self.linker.define(self.store, "env", "__cxa_end_catch",
            wasmtime.Func(self.store, wasmtime.FuncType([], []), cxa_end_catch))

        # __cxa_find_matching_catch_N - find a catch handler
        def make_find_matching_catch(n_types):
            def find_matching_catch(*args):
                thrown = self._exception_last
                if not thrown:
                    self._emscripten_tempret_set(self.store, 0)
                    return 0
                # For simplicity, always match the first type
                if args:
                    self._emscripten_tempret_set(self.store, args[0])
                else:
                    self._emscripten_tempret_set(self.store, 0)
                return thrown
            return find_matching_catch

        self.linker.define(self.store, "env", "__cxa_find_matching_catch_2",
            wasmtime.Func(self.store, wasmtime.FuncType([], [i32]), make_find_matching_catch(0)))
        self.linker.define(self.store, "env", "__cxa_find_matching_catch_3",
            wasmtime.Func(self.store, wasmtime.FuncType([i32], [i32]), make_find_matching_catch(1)))
        self.linker.define(self.store, "env", "__cxa_find_matching_catch_4",
            wasmtime.Func(self.store, wasmtime.FuncType([i32, i32], [i32]), make_find_matching_catch(2)))

        # __resumeException(ptr)
        def resume_exception(ptr):
            self._exception_last = ptr
            raise LongjmpException(f"__resumeException({ptr})")

        self.linker.define(self.store, "env", "__resumeException",
            wasmtime.Func(self.store, wasmtime.FuncType([i32], []), resume_exception))

        # llvm_eh_typeid_for(type) -> i32
        def llvm_eh_typeid_for(type_info):
            return type_info

        self.linker.define(self.store, "env", "llvm_eh_typeid_for",
            wasmtime.Func(self.store, wasmtime.FuncType([i32], [i32]), llvm_eh_typeid_for))

        # ---- Emscripten runtime stubs ----

        # emscripten_get_now() -> f64
        def emscripten_get_now():
            return time.monotonic() * 1000.0

        self.linker.define(self.store, "env", "emscripten_get_now",
            wasmtime.Func(self.store, wasmtime.FuncType([], [f64]), emscripten_get_now))

        # emscripten_date_now() -> f64
        def emscripten_date_now():
            return time.time() * 1000.0

        self.linker.define(self.store, "env", "emscripten_date_now",
            wasmtime.Func(self.store, wasmtime.FuncType([], [f64]), emscripten_date_now))

        # emscripten_resize_heap(requested_size) -> i32
        def emscripten_resize_heap(requested_size):
            return 0  # Fail; let WASM handle OOM

        self.linker.define(self.store, "env", "emscripten_resize_heap",
            wasmtime.Func(self.store, wasmtime.FuncType([i32], [i32]), emscripten_resize_heap))

        # _abort_js()
        def abort_js():
            raise RuntimeError("WASM aborted")

        self.linker.define(self.store, "env", "_abort_js",
            wasmtime.Func(self.store, wasmtime.FuncType([], []), abort_js))

        # _tzset_js, _localtime_js, _gmtime_js - time stubs
        def tzset_js(tz, dl, std, dst):
            pass  # Stub

        self.linker.define(self.store, "env", "_tzset_js",
            wasmtime.Func(self.store, wasmtime.FuncType([i32, i32, i32, i32], []), tzset_js))

        def localtime_js(time_val, tm_ptr):
            pass  # Stub

        self.linker.define(self.store, "env", "_localtime_js",
            wasmtime.Func(self.store, wasmtime.FuncType([i64, i32], []), localtime_js))

        def gmtime_js(time_val, tm_ptr):
            pass  # Stub

        self.linker.define(self.store, "env", "_gmtime_js",
            wasmtime.Func(self.store, wasmtime.FuncType([i64, i32], []), gmtime_js))

    def _read_string(self, ptr):
        """Read a null-terminated string from WASM memory."""
        if ptr == 0:
            return ""
        mem_data = self._memory.data_ptr(self.store)
        mem_len = self._memory.data_len(self.store)
        end = ptr
        while end < mem_len and mem_data[end] != 0:
            end += 1
        return bytes(mem_data[ptr:end]).decode("utf-8", errors="replace")

    def _write_string(self, s):
        """Write a string to WASM memory using stack allocation."""
        data = s.encode("utf-8") + b"\0"
        ptr = self._stackAlloc(self.store, len(data))
        mem_data = self._memory.data_ptr(self.store)
        for i, b in enumerate(data):
            mem_data[ptr + i] = b
        return ptr

    def execute(self, code):
        """
        Execute Luau code and return the output string.

        Returns a tuple of (output, error) where error is None on success.
        """
        if not self._initialized:
            raise RuntimeError("Not initialized")

        # Save stack before our string allocation
        stack = self._stackSave(self.store)

        try:
            # Write code to WASM memory
            code_ptr = self._write_string(code)

            # Call executeScript
            result_ptr = self._executeScript(self.store, code_ptr)

            # Read result
            result = self._read_string(result_ptr)

            if result.startswith("ERROR:"):
                return None, result[6:]
            else:
                return result, None

        except LongjmpException:
            return None, "Luau execution threw an exception"
        except wasmtime.Trap as e:
            return None, f"WASM trap: {e}"
        except Exception as e:
            return None, str(e)
        finally:
            # Restore stack
            self._stackRestore(self.store, stack)


def execute_luau(code, wasm_path=None):
    """One-shot Luau execution using wasmtime."""
    vm = LuauWasmtime(wasm_path)
    return vm.execute(code)


if __name__ == "__main__":
    print("Testing Luau wasmtime wrapper...")
    print()

    vm = LuauWasmtime()

    tests = [
        ("Hello World", 'print("Hello from Luau!")'),
        ("Arithmetic", 'print(2 + 2)'),
        ("Loop", 'for i = 1, 5 do print(i) end'),
        ("String interpolation", 'local x = 42\nprint(`The answer is {x}`)'),
        ("Fibonacci", '''local function fib(n)
    if n < 2 then return n end
    return fib(n-1) + fib(n-2)
end
print("fib(10) = " .. tostring(fib(10)))'''),
        ("Tables", '''local t = {name="Alice", age=30}
print("Name: " .. t.name)
print("Age: " .. tostring(t.age))'''),
        ("Error case", 'print(undefined_var.field)'),
        ("Math", 'print("pi = " .. tostring(math.pi))'),
        ("Table operations", '''local fruits = {"banana", "apple", "cherry"}
table.sort(fruits)
print(table.concat(fruits, ", "))'''),
    ]

    passed = 0
    failed = 0

    for name, code in tests:
        output, error = vm.execute(code)
        if error:
            print(f"  [{name}] Error: {error}")
            if name == "Error case":
                print(f"    (expected error)")
                passed += 1
            else:
                failed += 1
        else:
            print(f"  [{name}] Output: {output.strip()}")
            passed += 1

    print()
    print(f"Results: {passed} passed, {failed} failed out of {len(tests)} tests")
