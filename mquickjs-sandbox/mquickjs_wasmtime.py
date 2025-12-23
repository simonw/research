"""
mquickjs sandbox using wasmtime.

This implementation loads the WASM module directly using wasmtime.
"""

import struct
from pathlib import Path
from typing import Any, Optional

import wasmtime

from api_design import BaseSandbox, SandboxError, TimeoutError, MemoryError

SCRIPT_DIR = Path(__file__).parent
WASM_PATH = SCRIPT_DIR / "mquickjs_standalone.wasm"


class MQuickJSWasmtime(BaseSandbox):
    """
    mquickjs sandbox using wasmtime.

    This loads the standalone WASM module and provides Python bindings.
    Note: setjmp/longjmp based error handling requires special support.
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

        # Add stub functions for emscripten runtime
        self._add_emscripten_stubs()

        # Instantiate module
        try:
            self.instance = self.linker.instantiate(self.store, self.module)
        except wasmtime.WasmtimeError as e:
            raise SandboxError(f"Failed to instantiate WASM module: {e}")

        # Get exports
        self.memory = self.instance.exports(self.store)["memory"]
        self._sandbox_init = self.instance.exports(self.store)["sandbox_init"]
        self._sandbox_free = self.instance.exports(self.store)["sandbox_free"]
        self._sandbox_eval = self.instance.exports(self.store)["sandbox_eval"]
        self._sandbox_get_error = self.instance.exports(self.store)["sandbox_get_error"]
        self._malloc = self.instance.exports(self.store)["malloc"]
        self._free = self.instance.exports(self.store)["free"]

        # Initialize sandbox
        result = self._sandbox_init(self.store, memory_limit_bytes)
        if not result:
            raise MemoryError("Failed to initialize JavaScript context")

        self._initialized = True

    def _add_emscripten_stubs(self):
        """Add stub functions for emscripten runtime imports."""
        i32 = wasmtime.ValType.i32()

        # TempRet0 storage
        temp_ret0 = [0]

        def setTempRet0_fn(value):
            temp_ret0[0] = value

        def getTempRet0_fn():
            return temp_ret0[0]

        # Stub invoke functions - these are trampolines for indirect calls with exception handling
        # Without proper setjmp support, we just return 0 or do nothing

        def invoke_iii_fn(index, a1, a2):
            return 0

        def invoke_iiii_fn(index, a1, a2, a3):
            return 0

        def invoke_iiiii_fn(index, a1, a2, a3, a4):
            return 0

        def invoke_vi_fn(index, a1):
            pass

        def invoke_vii_fn(index, a1, a2):
            pass

        def invoke_viii_fn(index, a1, a2, a3):
            pass

        def invoke_viiiii_fn(index, a1, a2, a3, a4, a5):
            pass

        def invoke_viiiiii_fn(index, a1, a2, a3, a4, a5, a6):
            pass

        def emscripten_throw_longjmp_fn():
            raise RuntimeError("longjmp called")

        # Create Func objects and add to linker
        setTempRet0 = wasmtime.Func(self.store, wasmtime.FuncType([i32], []), setTempRet0_fn)
        getTempRet0 = wasmtime.Func(self.store, wasmtime.FuncType([], [i32]), getTempRet0_fn)
        invoke_iii = wasmtime.Func(self.store, wasmtime.FuncType([i32, i32, i32], [i32]), invoke_iii_fn)
        invoke_iiii = wasmtime.Func(self.store, wasmtime.FuncType([i32, i32, i32, i32], [i32]), invoke_iiii_fn)
        invoke_iiiii = wasmtime.Func(self.store, wasmtime.FuncType([i32, i32, i32, i32, i32], [i32]), invoke_iiiii_fn)
        invoke_vi = wasmtime.Func(self.store, wasmtime.FuncType([i32, i32], []), invoke_vi_fn)
        invoke_vii = wasmtime.Func(self.store, wasmtime.FuncType([i32, i32, i32], []), invoke_vii_fn)
        invoke_viii = wasmtime.Func(self.store, wasmtime.FuncType([i32, i32, i32, i32], []), invoke_viii_fn)
        invoke_viiiii = wasmtime.Func(self.store, wasmtime.FuncType([i32, i32, i32, i32, i32, i32], []), invoke_viiiii_fn)
        invoke_viiiiii = wasmtime.Func(self.store, wasmtime.FuncType([i32, i32, i32, i32, i32, i32, i32], []), invoke_viiiiii_fn)
        emscripten_throw_longjmp = wasmtime.Func(self.store, wasmtime.FuncType([], []), emscripten_throw_longjmp_fn)

        self.linker.define(self.store, "env", "setTempRet0", setTempRet0)
        self.linker.define(self.store, "env", "getTempRet0", getTempRet0)
        self.linker.define(self.store, "env", "invoke_iii", invoke_iii)
        self.linker.define(self.store, "env", "invoke_iiii", invoke_iiii)
        self.linker.define(self.store, "env", "invoke_iiiii", invoke_iiiii)
        self.linker.define(self.store, "env", "invoke_vi", invoke_vi)
        self.linker.define(self.store, "env", "invoke_vii", invoke_vii)
        self.linker.define(self.store, "env", "invoke_viii", invoke_viii)
        self.linker.define(self.store, "env", "invoke_viiiii", invoke_viiiii)
        self.linker.define(self.store, "env", "invoke_viiiiii", invoke_viiiiii)
        self.linker.define(self.store, "env", "_emscripten_throw_longjmp", emscripten_throw_longjmp)

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
