"""
Epsilon Python - Python bindings for the epsilon WebAssembly runtime

Epsilon is a pure Go WebAssembly runtime with zero dependencies that
fully supports the WebAssembly 2.0 specification.

Example usage:
    from epsilon import Runtime

    # Basic usage
    with Runtime() as runtime:
        with runtime.instantiate(wasm_bytes) as module:
            result = module.call("add", 5, 37)
            print(result)  # [42]

    # With memory limits (in pages, 1 page = 64KB)
    runtime = Runtime()
    module = runtime.instantiate(wasm_bytes, max_memory_pages=256)  # 16 MB max
"""

from .runtime import (
    Runtime,
    Module,
    EpsilonError,
    EpsilonTimeoutError,
    version,
    wrapper_version,
)

__version__ = "0.1.0"
__all__ = [
    "Runtime",
    "Module",
    "EpsilonError",
    "EpsilonTimeoutError",
    "version",
    "wrapper_version",
]
