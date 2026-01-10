"""Synchronous DenoBox implementation."""

from __future__ import annotations

import base64
import json
import subprocess
import threading
from pathlib import Path
from typing import Any, TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Self


class DenoBoxError(Exception):
    """Error raised when JavaScript execution fails."""

    pass


class WasmModule:
    """Wrapper for a loaded WebAssembly module."""

    def __init__(self, box: DenoBox, module_id: str, exports: dict[str, str]):
        self._box = box
        self._module_id = module_id
        self._exports = exports
        self._unloaded = False

    @property
    def module_id(self) -> str:
        """Get the module ID."""
        return self._module_id

    @property
    def exports(self) -> dict[str, str]:
        """Get the exported functions and their types."""
        return self._exports.copy()

    def call(self, func: str, *args: Any) -> Any:
        """Call an exported function.

        Args:
            func: Name of the function to call.
            *args: Arguments to pass to the function.

        Returns:
            The result of the function call.

        Raises:
            DenoBoxError: If the function call fails or module is unloaded.
        """
        if self._unloaded:
            raise DenoBoxError("WASM module has been unloaded")

        response = self._box._send_request(
            {
                "type": "call_wasm",
                "moduleId": self._module_id,
                "func": func,
                "args": list(args),
            }
        )

        if "error" in response:
            raise DenoBoxError(response["error"])

        return response.get("result")

    def unload(self) -> None:
        """Unload the WASM module."""
        if self._unloaded:
            return

        response = self._box._send_request(
            {
                "type": "unload_wasm",
                "moduleId": self._module_id,
            }
        )

        if "error" in response:
            raise DenoBoxError(response["error"])

        self._unloaded = True


class DenoBox:
    """Synchronous wrapper for executing JavaScript in a Deno sandbox.

    Usage:
        with DenoBox() as box:
            result = box.eval("1 + 1")
            print(result)  # 2
    """

    def __init__(self):
        self._process: subprocess.Popen | None = None
        self._request_id = 0
        self._lock = threading.Lock()
        self._worker_path = Path(__file__).parent / "worker.js"

    def start(self) -> None:
        """Start the Deno subprocess."""
        if self._process is not None:
            raise DenoBoxError("DenoBox already started")

        # Run Deno with no permissions - fully sandboxed
        # The worker communicates via stdin/stdout only
        self._process = subprocess.Popen(
            ["deno", "run", str(self._worker_path)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,  # Line buffered
        )

    def stop(self) -> None:
        """Stop the Deno subprocess."""
        if self._process is None:
            return

        try:
            # Send shutdown command
            self._send_request({"type": "shutdown"})
        except Exception:
            pass

        try:
            self._process.terminate()
            self._process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self._process.kill()
            self._process.wait()

        self._process = None

    def __enter__(self) -> "DenoBox":
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.stop()

    def _send_request(self, request: dict) -> dict:
        """Send a request to the Deno worker and get response."""
        if self._process is None:
            raise DenoBoxError("DenoBox not started")

        with self._lock:
            self._request_id += 1
            request["id"] = self._request_id

            # Send request
            request_line = json.dumps(request) + "\n"
            self._process.stdin.write(request_line)
            self._process.stdin.flush()

            # Read response
            response_line = self._process.stdout.readline()
            if not response_line:
                stderr = self._process.stderr.read()
                raise DenoBoxError(f"Deno process terminated unexpectedly: {stderr}")

            return json.loads(response_line)

    def eval(self, code: str) -> Any:
        """Evaluate JavaScript code and return the result.

        Args:
            code: JavaScript code to evaluate.

        Returns:
            The result of the evaluation, converted from JSON.

        Raises:
            DenoBoxError: If the JavaScript code raises an error.
        """
        response = self._send_request({"type": "eval", "code": code})

        if "error" in response:
            raise DenoBoxError(response["error"])

        return response.get("result")

    def load_wasm(
        self, path: str | None = None, wasm_bytes: bytes | None = None
    ) -> WasmModule:
        """Load a WebAssembly module.

        Args:
            path: Path to the WASM file (read by Python, not Deno).
            wasm_bytes: Raw WASM bytes to load directly.

        Returns:
            A WasmModule wrapper for calling exported functions.

        Raises:
            DenoBoxError: If loading the module fails.
            FileNotFoundError: If the path doesn't exist.
        """
        if wasm_bytes is None:
            if path is None:
                raise DenoBoxError("Either 'path' or 'wasm_bytes' must be provided")
            # Read the file in Python - Deno has no file system access
            wasm_bytes = Path(path).read_bytes()

        # Encode as base64 for JSON transport
        bytes_b64 = base64.b64encode(wasm_bytes).decode("ascii")

        response = self._send_request({"type": "load_wasm", "bytes": bytes_b64})

        if "error" in response:
            raise DenoBoxError(response["error"])

        result = response.get("result", {})
        return WasmModule(
            box=self,
            module_id=result["moduleId"],
            exports=result["exports"],
        )
