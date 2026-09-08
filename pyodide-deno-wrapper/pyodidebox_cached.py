"""PyodideBox Cached - Execute Python in Pyodide/Deno sandbox using pre-cached npm package."""

from __future__ import annotations

import json
import subprocess
import threading
from pathlib import Path
from typing import Any


class PyodideBoxError(Exception):
    """Error raised when Pyodide execution fails."""
    pass


class PyodideBox:
    """Synchronous wrapper for executing Python in a Pyodide/Deno sandbox.

    This version uses the Deno-cached npm:pyodide package. Run 'deno cache'
    first to cache the package, then this can run without network access.

    Usage:
        # First, cache pyodide (one-time, needs network):
        # deno cache --allow-import npm:pyodide@0.27.5

        with PyodideBox() as box:
            result = box.run("1 + 1")
            print(result)  # 2
    """

    def __init__(
        self,
        *,
        allow_read: bool = True,
        local_package_path: str | Path | None = None,
    ):
        """Initialize PyodideBox with cached Pyodide.

        Args:
            allow_read: Whether to allow file reading. Default True (required).
            local_package_path: Optional path where Pyodide packages are cached.
        """
        self._process: subprocess.Popen | None = None
        self._request_id = 0
        self._lock = threading.Lock()
        self._worker_path = Path(__file__).parent / "pyodide_cached_worker.js"
        self._allow_read = allow_read
        self._local_package_path = Path(local_package_path) if local_package_path else None
        self._initialized = False

    def start(self) -> None:
        """Start the Deno subprocess with cached Pyodide."""
        if self._process is not None:
            raise PyodideBoxError("PyodideBox already started")

        cmd = ["deno", "run", "--cached-only"]
        if self._allow_read:
            cmd.append("--allow-read")
        cmd.append(str(self._worker_path))

        self._process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )

        # Set local package path if provided
        if self._local_package_path:
            self._send_request({
                "type": "set_package_path",
                "path": str(self._local_package_path)
            })

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
        """Initialize Pyodide from cache.

        Returns:
            Dict with initialization info including Pyodide version.
        """
        response = self._send_request({"type": "init"})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        self._initialized = True
        return response.get("result", {})

    def run(self, code: str) -> Any:
        """Execute Python code and return the result."""
        response = self._send_request({"type": "run_python", "code": code})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result")

    def set_global(self, name: str, value: Any) -> None:
        """Set a global variable in the Pyodide namespace."""
        response = self._send_request({"type": "set_global", "name": name, "value": value})
        if "error" in response:
            raise PyodideBoxError(response["error"])

    def get_global(self, name: str) -> Any:
        """Get a global variable from the Pyodide namespace."""
        response = self._send_request({"type": "get_global", "name": name})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result")

    def install(self, *packages: str) -> dict:
        """Install Python packages from cache."""
        response = self._send_request({
            "type": "install_packages",
            "packages": list(packages)
        })
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result", {})

    def run_js(self, code: str) -> Any:
        """Execute JavaScript code in the Deno runtime."""
        response = self._send_request({"type": "run_js", "code": code})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result")


if __name__ == "__main__":
    print("Testing PyodideBox with cached npm:pyodide")
    print()

    with PyodideBox() as box:
        print("Initializing Pyodide from cache...")
        info = box.init()
        print(f"Pyodide version: {info['version']}")
        print()

        print("Testing basic math:")
        result = box.run("1 + 1")
        print(f"  1 + 1 = {result}")

        print()
        print("Testing function definition:")
        box.run('''
def greet(name):
    return f"Hello, {name}!"
''')
        result = box.run('greet("PyodideBox")')
        print(f"  greet('PyodideBox') = {result}")

        print()
        print("All tests passed!")
