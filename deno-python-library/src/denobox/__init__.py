"""denobox - Execute JavaScript and WebAssembly in a Deno sandbox from Python."""

from .async_box import AsyncDenoBox, AsyncWasmModule
from .sync_box import DenoBox, DenoBoxError, WasmModule

__all__ = [
    "DenoBox",
    "AsyncDenoBox",
    "DenoBoxError",
    "WasmModule",
    "AsyncWasmModule",
]
__version__ = "0.1.0"
