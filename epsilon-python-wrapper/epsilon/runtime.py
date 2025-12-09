"""
Epsilon Python Runtime

This module provides Python bindings to the epsilon Go library via ctypes.
"""

import ctypes
import os
import signal
import sys
from pathlib import Path
from typing import List, Optional, Union, Any


class EpsilonError(Exception):
    """Exception raised for epsilon-related errors"""
    pass


class EpsilonTimeoutError(EpsilonError):
    """Exception raised when execution times out"""
    pass


class _EpsilonLib:
    """Wrapper for the epsilon shared library"""

    def __init__(self):
        self._lib = None
        self._load_library()
        self._setup_functions()

    def _get_library_name(self) -> str:
        """Get the platform-appropriate library name"""
        if sys.platform == "darwin":
            return "libepsilon.dylib"
        elif sys.platform == "win32":
            return "libepsilon.dll"
        else:
            return "libepsilon.so"

    def _find_library(self) -> Path:
        """Find the libepsilon shared library"""
        lib_name = self._get_library_name()

        # Try common locations
        search_paths = [
            Path(__file__).parent / lib_name,  # Package dir (most common)
            Path(__file__).parent.parent / lib_name,  # Parent dir
            Path(lib_name),  # Current dir
        ]

        for path in search_paths:
            if path.exists():
                return path

        raise EpsilonError(
            f"Could not find {lib_name}. Searched: {search_paths}"
        )

    def _load_library(self):
        """Load the shared library"""
        lib_path = self._find_library()
        try:
            self._lib = ctypes.CDLL(str(lib_path))
        except OSError as e:
            raise EpsilonError(f"Failed to load library from {lib_path}: {e}")

    def _setup_functions(self):
        """Setup function signatures"""
        # epsilon_new_runtime(max_memory_pages) -> int64
        self._lib.epsilon_new_runtime.argtypes = [ctypes.c_uint32]
        self._lib.epsilon_new_runtime.restype = ctypes.c_longlong

        # epsilon_runtime_close(runtime_id)
        self._lib.epsilon_runtime_close.argtypes = [ctypes.c_longlong]
        self._lib.epsilon_runtime_close.restype = None

        # epsilon_instantiate(runtime_id, bytes, len) -> int64
        self._lib.epsilon_instantiate.argtypes = [
            ctypes.c_longlong,
            ctypes.POINTER(ctypes.c_uint8),
            ctypes.c_int
        ]
        self._lib.epsilon_instantiate.restype = ctypes.c_longlong

        # epsilon_instantiate_with_memory_limit
        self._lib.epsilon_instantiate_with_memory_limit.argtypes = [
            ctypes.c_longlong,
            ctypes.POINTER(ctypes.c_uint8),
            ctypes.c_int,
            ctypes.c_uint32
        ]
        self._lib.epsilon_instantiate_with_memory_limit.restype = ctypes.c_longlong

        # epsilon_module_close(module_id)
        self._lib.epsilon_module_close.argtypes = [ctypes.c_longlong]
        self._lib.epsilon_module_close.restype = None

        # epsilon_call_function
        self._lib.epsilon_call_function.argtypes = [
            ctypes.c_longlong,
            ctypes.c_char_p,
            ctypes.POINTER(ctypes.c_uint64),
            ctypes.c_int,
            ctypes.POINTER(ctypes.c_uint64),
            ctypes.c_int
        ]
        self._lib.epsilon_call_function.restype = ctypes.c_int

        # epsilon_call_function_i64
        self._lib.epsilon_call_function_i64.argtypes = [
            ctypes.c_longlong,
            ctypes.c_char_p,
            ctypes.POINTER(ctypes.c_uint64),
            ctypes.c_int,
            ctypes.POINTER(ctypes.c_uint8),
            ctypes.POINTER(ctypes.c_uint64),
            ctypes.c_int
        ]
        self._lib.epsilon_call_function_i64.restype = ctypes.c_int

        # epsilon_call_function_with_timeout
        self._lib.epsilon_call_function_with_timeout.argtypes = [
            ctypes.c_longlong,
            ctypes.c_char_p,
            ctypes.POINTER(ctypes.c_uint64),
            ctypes.c_int,
            ctypes.POINTER(ctypes.c_uint64),
            ctypes.c_int,
            ctypes.c_int64
        ]
        self._lib.epsilon_call_function_with_timeout.restype = ctypes.c_int

        # epsilon_get_export_names
        self._lib.epsilon_get_export_names.argtypes = [
            ctypes.c_longlong,
            ctypes.c_char_p,
            ctypes.c_int
        ]
        self._lib.epsilon_get_export_names.restype = ctypes.c_int

        # epsilon_get_memory_size
        self._lib.epsilon_get_memory_size.argtypes = [
            ctypes.c_longlong,
            ctypes.c_char_p
        ]
        self._lib.epsilon_get_memory_size.restype = ctypes.c_int32

        # epsilon_read_memory
        self._lib.epsilon_read_memory.argtypes = [
            ctypes.c_longlong,
            ctypes.c_char_p,
            ctypes.c_uint32,
            ctypes.c_uint32,
            ctypes.POINTER(ctypes.c_uint8)
        ]
        self._lib.epsilon_read_memory.restype = ctypes.c_int

        # epsilon_write_memory
        self._lib.epsilon_write_memory.argtypes = [
            ctypes.c_longlong,
            ctypes.c_char_p,
            ctypes.c_uint32,
            ctypes.POINTER(ctypes.c_uint8),
            ctypes.c_uint32
        ]
        self._lib.epsilon_write_memory.restype = ctypes.c_int

        # epsilon_get_global
        self._lib.epsilon_get_global.argtypes = [
            ctypes.c_longlong,
            ctypes.c_char_p,
            ctypes.POINTER(ctypes.c_uint64)
        ]
        self._lib.epsilon_get_global.restype = ctypes.c_int

        # epsilon_version() -> string
        self._lib.epsilon_version.argtypes = []
        self._lib.epsilon_version.restype = ctypes.c_char_p

        # epsilon_wrapper_version() -> string
        self._lib.epsilon_wrapper_version.argtypes = []
        self._lib.epsilon_wrapper_version.restype = ctypes.c_char_p

        # epsilon_get_error() -> string
        self._lib.epsilon_get_error.argtypes = []
        self._lib.epsilon_get_error.restype = ctypes.c_char_p

        # epsilon_free_string(str)
        self._lib.epsilon_free_string.argtypes = [ctypes.c_char_p]
        self._lib.epsilon_free_string.restype = None

    @property
    def lib(self):
        return self._lib


# Global library instance
_lib_instance = None


def _get_lib() -> _EpsilonLib:
    """Get or create the global library instance"""
    global _lib_instance
    if _lib_instance is None:
        _lib_instance = _EpsilonLib()
    return _lib_instance


def _get_last_error() -> str:
    """Get the last error message from the library"""
    lib = _get_lib().lib
    err = lib.epsilon_get_error()
    if err:
        msg = err.decode('utf-8')
        lib.epsilon_free_string(err)
        return msg
    return "Unknown error"


class Module:
    """A WebAssembly module instance"""

    def __init__(self, module_id: int, runtime_id: int):
        self._id = module_id
        self._runtime_id = runtime_id
        self._lib = _get_lib().lib
        self._closed = False

    def call(
        self,
        func_name: str,
        *args: int,
        timeout_ms: Optional[int] = None
    ) -> List[int]:
        """
        Call an exported function from the module

        Args:
            func_name: Name of the exported function
            *args: Integer arguments to pass (will be converted to int32)
            timeout_ms: Optional timeout in milliseconds (Unix only for full support)

        Returns:
            List of integer results

        Raises:
            EpsilonError: If the function call fails
            EpsilonTimeoutError: If execution times out
        """
        if self._closed:
            raise EpsilonError("Module is closed")

        # Prepare arguments
        nargs = len(args)
        args_array = (ctypes.c_uint64 * nargs)(*args) if nargs > 0 else None

        # Prepare results buffer (assume max 10 results)
        max_results = 10
        results_array = (ctypes.c_uint64 * max_results)()

        # Call the function
        if timeout_ms is not None and timeout_ms > 0:
            result_count = self._lib.epsilon_call_function_with_timeout(
                self._id,
                func_name.encode('utf-8'),
                args_array,
                nargs,
                results_array,
                max_results,
                timeout_ms
            )
        else:
            result_count = self._lib.epsilon_call_function(
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
                -3: f"Function call failed: {_get_last_error()}",
                -4: "Execution timeout"
            }
            msg = error_messages.get(result_count, f"Unknown error: {_get_last_error()}")
            if result_count == -4:
                raise EpsilonTimeoutError(msg)
            raise EpsilonError(msg)

        # Return results as a list
        return [int(results_array[i]) for i in range(result_count)]

    def call_typed(
        self,
        func_name: str,
        args: List[tuple],
        timeout_ms: Optional[int] = None
    ) -> List[int]:
        """
        Call an exported function with typed arguments

        Args:
            func_name: Name of the exported function
            args: List of (value, type) tuples where type is 'i32', 'i64', 'f32', or 'f64'
            timeout_ms: Optional timeout in milliseconds

        Returns:
            List of integer results

        Example:
            module.call_typed("compute", [(42, 'i32'), (3.14, 'f64')])
        """
        if self._closed:
            raise EpsilonError("Module is closed")

        type_map = {'i32': 0, 'i64': 1, 'f32': 2, 'f64': 3}

        nargs = len(args)
        args_array = (ctypes.c_uint64 * nargs)() if nargs > 0 else None
        types_array = (ctypes.c_uint8 * nargs)() if nargs > 0 else None

        for i, (value, vtype) in enumerate(args):
            if vtype not in type_map:
                raise EpsilonError(f"Unknown type: {vtype}")
            types_array[i] = type_map[vtype]

            if vtype == 'i32':
                args_array[i] = ctypes.c_uint64(ctypes.c_uint32(int(value)).value)
            elif vtype == 'i64':
                args_array[i] = ctypes.c_uint64(int(value))
            elif vtype == 'f32':
                import struct
                args_array[i] = struct.unpack('<I', struct.pack('<f', float(value)))[0]
            elif vtype == 'f64':
                import struct
                args_array[i] = struct.unpack('<Q', struct.pack('<d', float(value)))[0]

        max_results = 10
        results_array = (ctypes.c_uint64 * max_results)()

        result_count = self._lib.epsilon_call_function_i64(
            self._id,
            func_name.encode('utf-8'),
            args_array,
            nargs,
            types_array,
            results_array,
            max_results
        )

        if result_count < 0:
            error_messages = {
                -1: "Module not found",
                -2: f"Function '{func_name}' not found",
                -3: f"Function call failed: {_get_last_error()}",
                -4: "Execution timeout"
            }
            msg = error_messages.get(result_count, f"Unknown error: {_get_last_error()}")
            if result_count == -4:
                raise EpsilonTimeoutError(msg)
            raise EpsilonError(msg)

        return [int(results_array[i]) for i in range(result_count)]

    def get_export_names(self) -> List[str]:
        """Get the names of all exports from the module"""
        if self._closed:
            raise EpsilonError("Module is closed")

        # First, get required buffer size
        count = self._lib.epsilon_get_export_names(self._id, None, 0)
        if count < 0:
            raise EpsilonError(f"Failed to get export names: {_get_last_error()}")

        if count == 0:
            return []

        # Allocate buffer and get names
        buffer_size = count + 256  # Extra space for null separators
        buffer = ctypes.create_string_buffer(buffer_size)

        name_count = self._lib.epsilon_get_export_names(self._id, buffer, buffer_size)
        if name_count < 0:
            raise EpsilonError(f"Failed to get export names: {_get_last_error()}")

        # Parse null-separated names
        names_bytes = buffer.raw[:buffer.value.find(b'\x00\x00') if b'\x00\x00' in buffer.raw else len(buffer.value)]
        if not names_bytes:
            return []

        return [n.decode('utf-8') for n in names_bytes.split(b'\x00') if n]

    def get_memory_size(self, memory_name: str = "memory") -> int:
        """
        Get the size of an exported memory in pages (1 page = 64KB)

        Args:
            memory_name: Name of the exported memory (default: "memory")

        Returns:
            Memory size in pages
        """
        if self._closed:
            raise EpsilonError("Module is closed")

        size = self._lib.epsilon_get_memory_size(
            self._id,
            memory_name.encode('utf-8')
        )

        if size < 0:
            raise EpsilonError(f"Failed to get memory size: {_get_last_error()}")

        return size

    def read_memory(
        self,
        offset: int,
        length: int,
        memory_name: str = "memory"
    ) -> bytes:
        """
        Read bytes from the module's linear memory

        Args:
            offset: Byte offset in memory
            length: Number of bytes to read
            memory_name: Name of the exported memory (default: "memory")

        Returns:
            Bytes read from memory
        """
        if self._closed:
            raise EpsilonError("Module is closed")

        buffer = (ctypes.c_uint8 * length)()

        result = self._lib.epsilon_read_memory(
            self._id,
            memory_name.encode('utf-8'),
            offset,
            length,
            buffer
        )

        if result < 0:
            raise EpsilonError(f"Failed to read memory: {_get_last_error()}")

        return bytes(buffer)

    def write_memory(
        self,
        offset: int,
        data: bytes,
        memory_name: str = "memory"
    ) -> int:
        """
        Write bytes to the module's linear memory

        Args:
            offset: Byte offset in memory
            data: Bytes to write
            memory_name: Name of the exported memory (default: "memory")

        Returns:
            Number of bytes written
        """
        if self._closed:
            raise EpsilonError("Module is closed")

        data_array = (ctypes.c_uint8 * len(data)).from_buffer_copy(data)

        result = self._lib.epsilon_write_memory(
            self._id,
            memory_name.encode('utf-8'),
            offset,
            data_array,
            len(data)
        )

        if result < 0:
            raise EpsilonError(f"Failed to write memory: {_get_last_error()}")

        return result

    def get_global(self, global_name: str) -> int:
        """
        Get the value of an exported global variable

        Args:
            global_name: Name of the exported global

        Returns:
            The global's value as an integer
        """
        if self._closed:
            raise EpsilonError("Module is closed")

        result = ctypes.c_uint64()

        status = self._lib.epsilon_get_global(
            self._id,
            global_name.encode('utf-8'),
            ctypes.byref(result)
        )

        if status < 0:
            raise EpsilonError(f"Failed to get global: {_get_last_error()}")

        return result.value

    def close(self):
        """Close the module and free resources"""
        if not self._closed:
            self._lib.epsilon_module_close(self._id)
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
    """WebAssembly runtime using epsilon"""

    # Page size constant (64KB)
    PAGE_SIZE = 65536

    # Maximum pages constant (2GB / 64KB)
    MAX_PAGES = 32768

    def __init__(self, max_memory_pages: int = 0):
        """
        Create a new epsilon WebAssembly runtime

        Args:
            max_memory_pages: Default maximum memory pages for modules (0 = no limit)
                             1 page = 64KB, max = 32768 pages (2GB)
        """
        self._lib = _get_lib().lib
        self._max_memory_pages = max_memory_pages
        self._id = self._lib.epsilon_new_runtime(max_memory_pages)
        self._closed = False

        if self._id <= 0:
            raise EpsilonError(f"Failed to create runtime: {_get_last_error()}")

    def instantiate(
        self,
        wasm_bytes: Union[bytes, bytearray],
        max_memory_pages: Optional[int] = None
    ) -> Module:
        """
        Instantiate a WebAssembly module from bytes

        Args:
            wasm_bytes: The WebAssembly binary data
            max_memory_pages: Maximum memory pages for this module (overrides runtime default)
                             1 page = 64KB, max = 32768 pages (2GB)

        Returns:
            Module instance

        Raises:
            EpsilonError: If instantiation fails
        """
        if self._closed:
            raise EpsilonError("Runtime is closed")

        if not isinstance(wasm_bytes, (bytes, bytearray)):
            raise TypeError("wasm_bytes must be bytes or bytearray")

        # Convert to ctypes array
        wasm_array = (ctypes.c_uint8 * len(wasm_bytes)).from_buffer_copy(wasm_bytes)

        # Use specified memory limit or runtime default
        mem_limit = max_memory_pages if max_memory_pages is not None else self._max_memory_pages

        if mem_limit > 0:
            module_id = self._lib.epsilon_instantiate_with_memory_limit(
                self._id,
                wasm_array,
                len(wasm_bytes),
                mem_limit
            )
        else:
            module_id = self._lib.epsilon_instantiate(
                self._id,
                wasm_array,
                len(wasm_bytes)
            )

        if module_id < 0:
            raise EpsilonError(f"Failed to instantiate module: {_get_last_error()}")

        return Module(module_id, self._id)

    def instantiate_file(
        self,
        file_path: Union[str, Path],
        max_memory_pages: Optional[int] = None
    ) -> Module:
        """
        Instantiate a WebAssembly module from a file

        Args:
            file_path: Path to the .wasm file
            max_memory_pages: Maximum memory pages for this module

        Returns:
            Module instance

        Raises:
            EpsilonError: If instantiation fails
        """
        path = Path(file_path)
        if not path.exists():
            raise EpsilonError(f"File not found: {file_path}")

        with open(path, 'rb') as f:
            wasm_bytes = f.read()

        return self.instantiate(wasm_bytes, max_memory_pages)

    def close(self):
        """Close the runtime and free resources"""
        if not self._closed:
            self._lib.epsilon_runtime_close(self._id)
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
    """Get the epsilon library version"""
    lib = _get_lib().lib
    v = lib.epsilon_version()
    if v:
        return v.decode('utf-8')
    return "unknown"


def wrapper_version() -> str:
    """Get the Python wrapper version"""
    lib = _get_lib().lib
    v = lib.epsilon_wrapper_version()
    if v:
        return v.decode('utf-8')
    return "unknown"
