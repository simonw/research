"""
Wazero Python Runtime

This module provides Python bindings to the wazero Go library via ctypes.
"""

import ctypes
import os
from pathlib import Path
from typing import List, Optional, Union


class WazeroError(Exception):
    """Exception raised for wazero-related errors"""
    pass


class _WazeroLib:
    """Wrapper for the wazero shared library"""

    def __init__(self):
        self._lib = None
        self._load_library()
        self._setup_functions()

    def _find_library(self) -> Path:
        """Find the libwazero shared library"""
        # Try common locations
        search_paths = [
            Path(__file__).parent.parent / "libwazero.so",  # Package dir
            Path(__file__).parent / "libwazero.so",
            Path("libwazero.so"),  # Current dir
        ]

        for path in search_paths:
            if path.exists():
                return path

        raise WazeroError(
            f"Could not find libwazero.so. Searched: {search_paths}"
        )

    def _load_library(self):
        """Load the shared library"""
        lib_path = self._find_library()
        try:
            self._lib = ctypes.CDLL(str(lib_path))
        except OSError as e:
            raise WazeroError(f"Failed to load library from {lib_path}: {e}")

    def _setup_functions(self):
        """Setup function signatures"""
        # wazero_new_runtime() -> int64
        self._lib.wazero_new_runtime.argtypes = []
        self._lib.wazero_new_runtime.restype = ctypes.c_longlong

        # wazero_runtime_close(runtime_id)
        self._lib.wazero_runtime_close.argtypes = [ctypes.c_longlong]
        self._lib.wazero_runtime_close.restype = None

        # wazero_instantiate(runtime_id, bytes, len) -> int64
        self._lib.wazero_instantiate.argtypes = [
            ctypes.c_longlong,
            ctypes.POINTER(ctypes.c_uint8),
            ctypes.c_int
        ]
        self._lib.wazero_instantiate.restype = ctypes.c_longlong

        # wazero_module_close(module_id)
        self._lib.wazero_module_close.argtypes = [ctypes.c_longlong]
        self._lib.wazero_module_close.restype = None

        # wazero_call_function(module_id, name, args, nargs, results, maxresults) -> int
        self._lib.wazero_call_function.argtypes = [
            ctypes.c_longlong,
            ctypes.c_char_p,
            ctypes.POINTER(ctypes.c_uint64),
            ctypes.c_int,
            ctypes.POINTER(ctypes.c_uint64),
            ctypes.c_int
        ]
        self._lib.wazero_call_function.restype = ctypes.c_int

        # wazero_version() -> string
        self._lib.wazero_version.argtypes = []
        self._lib.wazero_version.restype = ctypes.c_char_p

    @property
    def lib(self):
        return self._lib


# Global library instance
_lib_instance = None

def _get_lib() -> _WazeroLib:
    """Get or create the global library instance"""
    global _lib_instance
    if _lib_instance is None:
        _lib_instance = _WazeroLib()
    return _lib_instance


class Module:
    """A WebAssembly module instance"""

    def __init__(self, module_id: int):
        self._id = module_id
        self._lib = _get_lib().lib
        self._closed = False

    def call(self, func_name: str, *args: int) -> List[int]:
        """
        Call an exported function from the module

        Args:
            func_name: Name of the exported function
            *args: Integer arguments to pass (will be converted to uint64)

        Returns:
            List of integer results

        Raises:
            WazeroError: If the function call fails
        """
        if self._closed:
            raise WazeroError("Module is closed")

        # Prepare arguments
        nargs = len(args)
        args_array = (ctypes.c_uint64 * nargs)(*args) if nargs > 0 else None

        # Prepare results buffer (assume max 10 results)
        max_results = 10
        results_array = (ctypes.c_uint64 * max_results)()

        # Call the function
        result_count = self._lib.wazero_call_function(
            self._id,
            func_name.encode('utf-8'),
            args_array,
            nargs,
            results_array,
            max_results
        )

        if result_count < 0:
            error_messages = {
                -1: "Module not found",
                -2: f"Function '{func_name}' not found",
                -3: "Function call failed"
            }
            raise WazeroError(error_messages.get(result_count, "Unknown error"))

        # Return results as a list
        return [int(results_array[i]) for i in range(result_count)]

    def close(self):
        """Close the module and free resources"""
        if not self._closed:
            self._lib.wazero_module_close(self._id)
            self._closed = True

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
        return False

    def __del__(self):
        if not self._closed:
            self.close()


class Runtime:
    """WebAssembly runtime using wazero"""

    def __init__(self):
        self._lib = _get_lib().lib
        self._id = self._lib.wazero_new_runtime()
        self._closed = False

        if self._id <= 0:
            raise WazeroError("Failed to create runtime")

    def instantiate(self, wasm_bytes: Union[bytes, bytearray]) -> Module:
        """
        Instantiate a WebAssembly module from bytes

        Args:
            wasm_bytes: The WebAssembly binary data

        Returns:
            Module instance

        Raises:
            WazeroError: If instantiation fails
        """
        if self._closed:
            raise WazeroError("Runtime is closed")

        if not isinstance(wasm_bytes, (bytes, bytearray)):
            raise TypeError("wasm_bytes must be bytes or bytearray")

        # Convert to ctypes array
        wasm_array = (ctypes.c_uint8 * len(wasm_bytes)).from_buffer_copy(wasm_bytes)

        # Instantiate the module
        module_id = self._lib.wazero_instantiate(
            self._id,
            wasm_array,
            len(wasm_bytes)
        )

        if module_id < 0:
            raise WazeroError("Failed to instantiate module")

        return Module(module_id)

    def instantiate_file(self, file_path: Union[str, Path]) -> Module:
        """
        Instantiate a WebAssembly module from a file

        Args:
            file_path: Path to the .wasm file

        Returns:
            Module instance

        Raises:
            WazeroError: If instantiation fails
        """
        path = Path(file_path)
        if not path.exists():
            raise WazeroError(f"File not found: {file_path}")

        with open(path, 'rb') as f:
            wasm_bytes = f.read()

        return self.instantiate(wasm_bytes)

    def close(self):
        """Close the runtime and free resources"""
        if not self._closed:
            self._lib.wazero_runtime_close(self._id)
            self._closed = True

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
        return False

    def __del__(self):
        if not self._closed:
            self.close()


def version() -> str:
    """Get the wazero bindings version"""
    lib = _get_lib().lib
    return lib.wazero_version().decode('utf-8')
