"""
mquickjs sandbox for Pyodide (browser/WebWorker environment).

This module provides a Python API for mquickjs when running in Pyodide.
It uses the WASM module compiled with emscripten.

Usage in Pyodide:
    import mquickjs_pyodide
    result = await mquickjs_pyodide.execute_js("1 + 2")
"""

# This module is designed to run in Pyodide, where we have access to the
# JavaScript runtime and can load WASM modules.

try:
    from js import fetch, Uint8Array
    from pyodide.ffi import to_js
    IN_PYODIDE = True
except ImportError:
    IN_PYODIDE = False


_module = None
_sandbox_init = None
_sandbox_free = None
_sandbox_eval = None
_sandbox_get_error = None


async def _ensure_loaded():
    """Load the WASM module if not already loaded."""
    global _module, _sandbox_init, _sandbox_free, _sandbox_eval, _sandbox_get_error

    if _module is not None:
        return

    if not IN_PYODIDE:
        raise RuntimeError("This module requires Pyodide")

    from js import eval as js_eval

    # Load the emscripten module
    # First, load the JS file
    js_code = """
    (async function() {
        // Fetch the JS module
        const jsResponse = await fetch('./mquickjs.js');
        const jsText = await jsResponse.text();

        // Eval the module factory
        const createModule = eval(jsText + '; createMQuickJS;');

        // Fetch the WASM binary
        const wasmResponse = await fetch('./mquickjs.wasm');
        const wasmBinary = await wasmResponse.arrayBuffer();

        // Create the module instance
        const Module = await createModule({ wasmBinary: new Uint8Array(wasmBinary) });

        return Module;
    })()
    """
    _module = await js_eval(js_code)

    # Get function wrappers
    _sandbox_init = _module.cwrap('sandbox_init', 'number', ['number'])
    _sandbox_free = _module.cwrap('sandbox_free', None, [])
    _sandbox_eval = _module.cwrap('sandbox_eval', 'string', ['string'])
    _sandbox_get_error = _module.cwrap('sandbox_get_error', 'string', [])


async def init_sandbox(memory_size: int = 1024 * 1024) -> bool:
    """Initialize the sandbox with given memory size."""
    await _ensure_loaded()
    result = _sandbox_init(memory_size)
    return bool(result)


def free_sandbox():
    """Free the sandbox resources."""
    if _sandbox_free is not None:
        _sandbox_free()


async def execute_js(code: str) -> str:
    """Execute JavaScript code and return the result as a string."""
    await _ensure_loaded()

    # Initialize if needed
    if _sandbox_init is not None:
        result = _sandbox_eval(code)
        if result is None:
            error = _sandbox_get_error()
            raise RuntimeError(f"JavaScript error: {error}")
        return result
    else:
        raise RuntimeError("Sandbox not initialized")


# Cleanup function
def cleanup():
    """Clean up the sandbox."""
    free_sandbox()
