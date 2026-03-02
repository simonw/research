"""Synchronous PyodideBox implementation."""

from __future__ import annotations

import json
import subprocess
import threading
from pathlib import Path
from typing import Any, TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Self


class PyodideBoxError(Exception):
    """Error raised when Pyodide execution fails."""
    pass


class PyodideBox:
    """Synchronous wrapper for executing Python in a Pyodide/Deno sandbox.

    This runs Python code inside Pyodide (Python compiled to WebAssembly),
    running inside Deno, providing a sandboxed Python execution environment.

    Usage:
        with PyodideBox() as box:
            result = box.run("1 + 1")
            print(result)  # 2

            # Run more complex code
            box.run('''
                def greet(name):
                    return f"Hello, {name}!"
            ''')
            result = box.run('greet("World")')
            print(result)  # "Hello, World!"
    """

    def __init__(
        self,
        *,
        allow_net: bool = True,
        allow_read: bool = True,
        ignore_cert_errors: bool = False
    ):
        """Initialize PyodideBox.

        Args:
            allow_net: Whether to allow network access (needed to download Pyodide).
                       Default is True. Set to False if using a local Pyodide install.
            allow_read: Whether to allow file reading (needed for Pyodide to read cached
                        WASM files). Default is True.
            ignore_cert_errors: Whether to ignore certificate errors (useful for testing
                                in environments with certificate issues). Default is False.
        """
        self._process: subprocess.Popen | None = None
        self._request_id = 0
        self._lock = threading.Lock()
        self._worker_path = Path(__file__).parent.parent / "pyodide_worker.js"
        self._allow_net = allow_net
        self._allow_read = allow_read
        self._ignore_cert_errors = ignore_cert_errors
        self._initialized = False

    def start(self) -> None:
        """Start the Deno subprocess with Pyodide."""
        if self._process is not None:
            raise PyodideBoxError("PyodideBox already started")

        cmd = ["deno", "run"]
        if self._allow_net:
            cmd.append("--allow-net")
        if self._allow_read:
            cmd.append("--allow-read")
        if self._ignore_cert_errors:
            cmd.append("--unsafely-ignore-certificate-errors")
        cmd.append(str(self._worker_path))

        self._process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )

    def stop(self) -> None:
        """Stop the Deno subprocess."""
        if self._process is None:
            return

        try:
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
        self._initialized = False

    def __enter__(self) -> "PyodideBox":
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.stop()

    def _send_request(self, request: dict, timeout: float | None = None) -> dict:
        """Send a request to the Deno worker and get response."""
        if self._process is None:
            raise PyodideBoxError("PyodideBox not started")

        with self._lock:
            self._request_id += 1
            request["id"] = self._request_id

            request_line = json.dumps(request) + "\n"
            self._process.stdin.write(request_line)
            self._process.stdin.flush()

            response_line = self._process.stdout.readline()
            if not response_line:
                stderr = self._process.stderr.read()
                raise PyodideBoxError(f"Deno process terminated unexpectedly: {stderr}")

            return json.loads(response_line)

    def init(self) -> dict:
        """Initialize Pyodide (downloads and loads it).

        This is called automatically on first run(), but you can call it
        explicitly if you want to pre-initialize.

        Returns:
            Dict with initialization info including Pyodide version.
        """
        response = self._send_request({"type": "init"})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        self._initialized = True
        return response.get("result", {})

    def run(self, code: str) -> Any:
        """Execute Python code and return the result.

        Args:
            code: Python code to execute.

        Returns:
            The result of the last expression, converted to Python types.

        Raises:
            PyodideBoxError: If execution fails.
        """
        response = self._send_request({"type": "run_python", "code": code})

        if "error" in response:
            raise PyodideBoxError(response["error"])

        return response.get("result")

    def set_global(self, name: str, value: Any) -> None:
        """Set a global variable in the Pyodide namespace.

        Args:
            name: Variable name.
            value: Value to set (must be JSON-serializable).
        """
        response = self._send_request({"type": "set_global", "name": name, "value": value})
        if "error" in response:
            raise PyodideBoxError(response["error"])

    def get_global(self, name: str) -> Any:
        """Get a global variable from the Pyodide namespace.

        Args:
            name: Variable name.

        Returns:
            The value of the variable.
        """
        response = self._send_request({"type": "get_global", "name": name})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result")

    def install(self, *packages: str) -> dict:
        """Install Python packages using micropip.

        Args:
            *packages: Package names to install.

        Returns:
            Dict with installed package names.
        """
        response = self._send_request({
            "type": "install_packages",
            "packages": list(packages)
        })
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result", {})

    def run_js(self, code: str) -> Any:
        """Execute JavaScript code in the Deno runtime.

        Args:
            code: JavaScript code to execute.

        Returns:
            The result of the expression.
        """
        response = self._send_request({"type": "run_js", "code": code})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result")
