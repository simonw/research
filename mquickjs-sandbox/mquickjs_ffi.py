"""
mquickjs FFI bindings using ctypes.

This module provides Python bindings to mquickjs using the shared library.
"""

import ctypes
import os
from pathlib import Path
from typing import Any, Optional

from api_design import BaseSandbox, SandboxError, TimeoutError, MemoryError


# Load the shared library
_lib_path = Path(__file__).parent / "libmquickjs_sandbox.so"
if not _lib_path.exists():
    raise ImportError(f"Shared library not found: {_lib_path}. Run build_ffi.py first.")

_lib = ctypes.CDLL(str(_lib_path))

# Define function signatures
_lib.sandbox_new.argtypes = [ctypes.c_size_t, ctypes.c_int64]
_lib.sandbox_new.restype = ctypes.c_void_p

_lib.sandbox_free.argtypes = [ctypes.c_void_p]
_lib.sandbox_free.restype = None

_lib.sandbox_eval.argtypes = [
    ctypes.c_void_p,  # context
    ctypes.c_char_p,  # code
    ctypes.c_size_t,  # code_len
    ctypes.c_char_p,  # result_buf
    ctypes.c_size_t,  # result_buf_size
    ctypes.c_char_p,  # error_buf
    ctypes.c_size_t,  # error_buf_size
]
_lib.sandbox_eval.restype = ctypes.c_int

_lib.sandbox_timed_out.argtypes = [ctypes.c_void_p]
_lib.sandbox_timed_out.restype = ctypes.c_int


class MQuickJSFFI(BaseSandbox):
    """
    mquickjs sandbox using FFI (ctypes).

    This implementation uses a shared library built from mquickjs.
    """

    def __init__(
        self,
        memory_limit_bytes: int = BaseSandbox.DEFAULT_MEMORY_LIMIT,
        time_limit_ms: int = BaseSandbox.DEFAULT_TIME_LIMIT_MS,
    ):
        super().__init__(memory_limit_bytes, time_limit_ms)
        self._ctx = _lib.sandbox_new(memory_limit_bytes, time_limit_ms)
        if not self._ctx:
            raise MemoryError("Failed to create sandbox context")

    def __del__(self):
        if hasattr(self, '_ctx') and self._ctx:
            _lib.sandbox_free(self._ctx)
            self._ctx = None

    def execute(self, code: str) -> Any:
        """Execute JavaScript code and return the result."""
        if not self._ctx:
            raise SandboxError("Sandbox context is closed")

        code_bytes = code.encode('utf-8')

        # Allocate buffers for result and error
        result_buf = ctypes.create_string_buffer(65536)
        error_buf = ctypes.create_string_buffer(4096)

        ret = _lib.sandbox_eval(
            self._ctx,
            code_bytes,
            len(code_bytes),
            result_buf,
            len(result_buf),
            error_buf,
            len(error_buf),
        )

        if ret == 2:  # Timeout
            raise TimeoutError(error_buf.value.decode('utf-8', errors='replace'))
        elif ret == 3:  # Memory error
            raise MemoryError(error_buf.value.decode('utf-8', errors='replace'))
        elif ret != 0:  # Other error
            raise SandboxError(error_buf.value.decode('utf-8', errors='replace'))

        # Parse result
        result_str = result_buf.value.decode('utf-8', errors='replace')
        return self._parse_result(result_str)

    def _parse_result(self, result_str: str) -> Any:
        """Parse the result string into a Python value."""
        if result_str == "undefined":
            return None
        elif result_str == "null":
            return None
        elif result_str == "true":
            return True
        elif result_str == "false":
            return False
        elif result_str.startswith("[object"):
            return result_str  # Return as string for objects
        else:
            # Try to parse as number
            try:
                if '.' in result_str or 'e' in result_str.lower():
                    return float(result_str)
                else:
                    return int(result_str)
            except ValueError:
                # Return as string
                return result_str

    def close(self):
        """Explicitly close the sandbox context."""
        if self._ctx:
            _lib.sandbox_free(self._ctx)
            self._ctx = None


def execute_js(
    code: str,
    *,
    memory_limit_bytes: int = BaseSandbox.DEFAULT_MEMORY_LIMIT,
    time_limit_ms: int = BaseSandbox.DEFAULT_TIME_LIMIT_MS,
) -> Any:
    """
    Execute JavaScript code in a sandbox.

    This creates a temporary sandbox, executes the code, and returns the result.
    """
    sandbox = MQuickJSFFI(memory_limit_bytes, time_limit_ms)
    try:
        return sandbox.execute(code)
    finally:
        sandbox.close()


if __name__ == "__main__":
    # Quick test
    print("Testing mquickjs FFI bindings...")

    # Basic arithmetic
    result = execute_js("1 + 2")
    print(f"1 + 2 = {result}")
    assert result == 3

    # String
    result = execute_js("'hello' + ' ' + 'world'")
    print(f"'hello' + ' ' + 'world' = {result}")
    assert result == "hello world"

    # Boolean
    result = execute_js("true && false")
    print(f"true && false = {result}")
    assert result == False

    # Function
    result = execute_js("(function() { var sum = 0; for (var i = 0; i < 10; i++) sum += i; return sum; })()")
    print(f"Sum 0-9 = {result}")
    assert result == 45

    print("All tests passed!")
