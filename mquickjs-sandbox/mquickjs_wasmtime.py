"""
mquickjs sandbox using wasmtime.

This implementation loads the WASM module directly using wasmtime and
implements proper invoke_* trampolines for setjmp/longjmp support.
"""

import struct
from pathlib import Path
from typing import Any, Optional, List

import wasmtime

from api_design import BaseSandbox, SandboxError, TimeoutError, MemoryError

SCRIPT_DIR = Path(__file__).parent
WASM_PATH = SCRIPT_DIR / "mquickjs_standalone.wasm"


class SetjmpLongjmpHandler:
    """
    Handler for emscripten's setjmp/longjmp mechanism.

    Emscripten uses a table-based approach for setjmp:
    - setjmp stores info in a buffer and returns 0
    - longjmp looks up the info and calls _emscripten_throw_longjmp
    - invoke_* functions catch the longjmp and return control

    The invoke_* pattern:
    1. Save stack state
    2. Call function via indirect call
    3. If longjmp is thrown, catch it and set threw flag
    4. Restore stack and return
    """

    def __init__(self):
        self.threw = 0
        self.threw_value = 0
        self.temp_ret0 = 0

    def set_threw(self, value, type_val):
        """Called when longjmp is executed."""
        if self.threw == 0:
            self.threw = value
            self.threw_value = type_val

    def clear_threw(self):
        """Clear the threw flag."""
        self.threw = 0
        self.threw_value = 0


class MQuickJSWasmtime(BaseSandbox):
    """
    mquickjs sandbox using wasmtime.

    This loads the standalone WASM module and provides Python bindings
    with proper setjmp/longjmp support via invoke_* trampolines.
    """

    def __init__(
        self,
        memory_limit_bytes: int = BaseSandbox.DEFAULT_MEMORY_LIMIT,
        time_limit_ms: int = BaseSandbox.DEFAULT_TIME_LIMIT_MS,
    ):
        super().__init__(memory_limit_bytes, time_limit_ms)

        # Create wasmtime engine and store
        self.engine = wasmtime.Engine()
        self.store = wasmtime.Store(self.engine)

        # Load WASM module
        wasm_bytes = WASM_PATH.read_bytes()
        self.module = wasmtime.Module(self.engine, wasm_bytes)

        # Create WASI config
        wasi_config = wasmtime.WasiConfig()
        wasi_config.inherit_stdout()
        wasi_config.inherit_stderr()
        self.store.set_wasi(wasi_config)

        # Create linker and add WASI
        self.linker = wasmtime.Linker(self.engine)
        self.linker.define_wasi()

        # Setjmp/longjmp handler
        self.sjlj = SetjmpLongjmpHandler()

        # Add emscripten runtime functions
        self._add_emscripten_runtime()

        # Instantiate module
        try:
            self.instance = self.linker.instantiate(self.store, self.module)
        except wasmtime.WasmtimeError as e:
            raise SandboxError(f"Failed to instantiate WASM module: {e}")

        # Get exports
        exports = self.instance.exports(self.store)
        self.memory = exports["memory"]
        self._sandbox_init = exports["sandbox_init"]
        self._sandbox_free = exports["sandbox_free"]
        self._sandbox_eval = exports["sandbox_eval"]
        self._sandbox_get_error = exports["sandbox_get_error"]
        self._malloc = exports["malloc"]
        self._free = exports["free"]

        # Get the indirect function table for invoke_* calls
        self._table = exports.get("__indirect_function_table")

        # Get setjmp helper exports
        self._setThrew = exports.get("setThrew")
        self._saveSetjmp = exports.get("saveSetjmp")
        self._stackSave = exports.get("stackSave")
        self._stackRestore = exports.get("stackRestore")

        # Initialize sandbox
        result = self._sandbox_init(self.store, memory_limit_bytes)
        if not result:
            raise MemoryError("Failed to initialize JavaScript context")

        self._initialized = True

    def _add_emscripten_runtime(self):
        """Add emscripten runtime functions with proper invoke_* support."""
        i32 = wasmtime.ValType.i32()

        # TempRet0 - used for returning 64-bit values
        def setTempRet0_fn(value):
            self.sjlj.temp_ret0 = value

        def getTempRet0_fn():
            return self.sjlj.temp_ret0

        # _emscripten_throw_longjmp - called by longjmp
        def emscripten_throw_longjmp_fn():
            # This signals that longjmp was called
            # The invoke_* wrapper will catch this
            raise LongjmpException()

        # Create invoke_* functions that properly call through the indirect table
        def make_invoke_fn(return_type: str, param_count: int):
            """Create an invoke function with the given signature."""

            def invoke_fn(*args):
                # args[0] is the function index, rest are parameters
                index = args[0]
                params = args[1:]

                # Save stack
                stack_ptr = None
                if self._stackSave:
                    try:
                        stack_ptr = self._stackSave(self.store)
                    except:
                        pass

                try:
                    # Get the function from the indirect table
                    if self._table is None:
                        # No table available, return default
                        return 0 if return_type == 'i' else None

                    # Call through the table
                    func = self._table.get(self.store, index)
                    if func is None:
                        return 0 if return_type == 'i' else None

                    # Call the function
                    result = func(self.store, *params)

                    # Clear threw flag on successful return
                    if self._setThrew:
                        self._setThrew(self.store, 0, 0)

                    return result if return_type == 'i' else None

                except LongjmpException:
                    # longjmp was called, set the threw flag
                    if self._setThrew:
                        self._setThrew(self.store, 1, 0)

                    # Restore stack
                    if stack_ptr is not None and self._stackRestore:
                        try:
                            self._stackRestore(self.store, stack_ptr)
                        except:
                            pass

                    return 0 if return_type == 'i' else None

                except wasmtime.Trap as e:
                    # WASM trap (could be longjmp or actual error)
                    if "unreachable" in str(e):
                        # Likely a longjmp that reached unreachable
                        if self._setThrew:
                            try:
                                self._setThrew(self.store, 1, 0)
                            except:
                                pass

                    # Restore stack
                    if stack_ptr is not None and self._stackRestore:
                        try:
                            self._stackRestore(self.store, stack_ptr)
                        except:
                            pass

                    return 0 if return_type == 'i' else None

                except Exception as e:
                    # Other error
                    return 0 if return_type == 'i' else None

            return invoke_fn

        # Create and register invoke functions
        invoke_fns = {
            # invoke_iii(index, a1, a2) -> i32
            "invoke_iii": (wasmtime.FuncType([i32, i32, i32], [i32]), make_invoke_fn('i', 2)),
            # invoke_iiii(index, a1, a2, a3) -> i32
            "invoke_iiii": (wasmtime.FuncType([i32, i32, i32, i32], [i32]), make_invoke_fn('i', 3)),
            # invoke_iiiii(index, a1, a2, a3, a4) -> i32
            "invoke_iiiii": (wasmtime.FuncType([i32, i32, i32, i32, i32], [i32]), make_invoke_fn('i', 4)),
            # invoke_vi(index, a1)
            "invoke_vi": (wasmtime.FuncType([i32, i32], []), make_invoke_fn('v', 1)),
            # invoke_vii(index, a1, a2)
            "invoke_vii": (wasmtime.FuncType([i32, i32, i32], []), make_invoke_fn('v', 2)),
            # invoke_viii(index, a1, a2, a3)
            "invoke_viii": (wasmtime.FuncType([i32, i32, i32, i32], []), make_invoke_fn('v', 3)),
            # invoke_viiiii(index, a1, a2, a3, a4, a5)
            "invoke_viiiii": (wasmtime.FuncType([i32, i32, i32, i32, i32, i32], []), make_invoke_fn('v', 5)),
            # invoke_viiiiii(index, a1, a2, a3, a4, a5, a6)
            "invoke_viiiiii": (wasmtime.FuncType([i32, i32, i32, i32, i32, i32, i32], []), make_invoke_fn('v', 6)),
        }

        # Register all functions
        setTempRet0 = wasmtime.Func(self.store, wasmtime.FuncType([i32], []), setTempRet0_fn)
        getTempRet0 = wasmtime.Func(self.store, wasmtime.FuncType([], [i32]), getTempRet0_fn)
        emscripten_throw_longjmp = wasmtime.Func(
            self.store, wasmtime.FuncType([], []), emscripten_throw_longjmp_fn
        )

        self.linker.define(self.store, "env", "setTempRet0", setTempRet0)
        self.linker.define(self.store, "env", "getTempRet0", getTempRet0)
        self.linker.define(self.store, "env", "_emscripten_throw_longjmp", emscripten_throw_longjmp)

        for name, (func_type, func) in invoke_fns.items():
            self.linker.define(
                self.store, "env", name,
                wasmtime.Func(self.store, func_type, func)
            )

    def _read_string(self, ptr: int) -> str:
        """Read a null-terminated string from WASM memory."""
        if ptr == 0:
            return ""
        mem_data = self.memory.data_ptr(self.store)
        mem_len = self.memory.data_len(self.store)

        # Find null terminator
        end = ptr
        while end < mem_len and mem_data[end] != 0:
            end += 1

        return bytes(mem_data[ptr:end]).decode("utf-8", errors="replace")

    def _write_string(self, s: str) -> int:
        """Write a string to WASM memory and return pointer."""
        data = s.encode("utf-8") + b"\0"
        ptr = self._malloc(self.store, len(data))
        if ptr == 0:
            raise MemoryError("Failed to allocate memory for string")

        mem_data = self.memory.data_ptr(self.store)
        for i, b in enumerate(data):
            mem_data[ptr + i] = b

        return ptr

    def execute(self, code: str) -> Any:
        """Execute JavaScript code and return the result."""
        if not self._initialized:
            raise SandboxError("Sandbox not initialized")

        # Write code to WASM memory
        code_ptr = self._write_string(code)

        try:
            # Call sandbox_eval
            result_ptr = self._sandbox_eval(self.store, code_ptr)

            if result_ptr == 0:
                # Error occurred
                error_ptr = self._sandbox_get_error(self.store)
                error_msg = self._read_string(error_ptr)
                raise SandboxError(error_msg or "Unknown error")

            # Read result string
            result_str = self._read_string(result_ptr)
            return self._parse_result(result_str)

        finally:
            self._free(self.store, code_ptr)

    def _parse_result(self, result_str: str) -> Any:
        """Parse result string to Python value."""
        if result_str == "undefined" or result_str == "null":
            return None
        if result_str == "true":
            return True
        if result_str == "false":
            return False

        # Try integer
        try:
            return int(result_str)
        except ValueError:
            pass

        # Try float
        try:
            return float(result_str)
        except ValueError:
            pass

        return result_str

    def close(self):
        """Close the sandbox and free resources."""
        if hasattr(self, "_initialized") and self._initialized:
            self._sandbox_free(self.store)
            self._initialized = False


class LongjmpException(Exception):
    """Exception raised when longjmp is called in WASM."""
    pass


def execute_js(
    code: str,
    *,
    memory_limit_bytes: int = BaseSandbox.DEFAULT_MEMORY_LIMIT,
    time_limit_ms: int = BaseSandbox.DEFAULT_TIME_LIMIT_MS,
) -> Any:
    """Execute JavaScript code using wasmtime."""
    sandbox = MQuickJSWasmtime(memory_limit_bytes, time_limit_ms)
    try:
        return sandbox.execute(code)
    finally:
        sandbox.close()


if __name__ == "__main__":
    print("Testing mquickjs wasmtime wrapper...")

    try:
        # Basic arithmetic
        result = execute_js("1 + 2")
        print(f"1 + 2 = {result}")
        assert result == 3

        # String
        result = execute_js("'hello'")
        print(f"String = {result}")
        assert result == "hello"

        print("All wasmtime tests passed!")
    except Exception as e:
        print(f"Error: {e}")
        import traceback

        traceback.print_exc()
