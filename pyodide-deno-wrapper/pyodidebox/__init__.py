"""PyodideBox - Execute Python in a Pyodide/Deno sandbox from Python."""

from .sync_box import PyodideBox, PyodideBoxError
from .async_box import AsyncPyodideBox

__all__ = ["PyodideBox", "AsyncPyodideBox", "PyodideBoxError"]
__version__ = "0.1.0"
