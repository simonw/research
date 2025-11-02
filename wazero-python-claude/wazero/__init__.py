"""
Wazero Python Bindings

Python bindings for the wazero WebAssembly runtime (https://wazero.io/)
"""

from .runtime import Runtime, Module, WazeroError, version

__version__ = "0.1.0"
__all__ = ["Runtime", "Module", "WazeroError", "version"]
