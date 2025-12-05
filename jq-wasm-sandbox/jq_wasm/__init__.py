"""
jq WASM Sandbox - Execute jq programs in a WebAssembly sandbox

This library provides a secure way to run jq (or jaq) programs with:
- Memory limits
- CPU/instruction limits (fuel)
- No filesystem access (sandboxed)

Supports multiple WASM runtimes:
- wasmtime
- wasmer
"""

from .wasmtime_runner import WasmtimeJqRunner
from .wasmer_runner import WasmerJqRunner
from .base import JqRunner, JqError, JqTimeoutError, JqMemoryError

__version__ = "0.1.0"
__all__ = [
    "WasmtimeJqRunner",
    "WasmerJqRunner",
    "JqRunner",
    "JqError",
    "JqTimeoutError",
    "JqMemoryError",
]
