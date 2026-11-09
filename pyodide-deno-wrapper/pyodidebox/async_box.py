"""Asynchronous PyodideBox implementation."""

from __future__ import annotations

import asyncio
import json
from pathlib import Path
from typing import Any, TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Self

from .sync_box import PyodideBoxError


class AsyncPyodideBox:
    """Asynchronous wrapper for executing Python in a Pyodide/Deno sandbox.

    This runs Python code inside Pyodide (Python compiled to WebAssembly),
    running inside Deno, providing a sandboxed Python execution environment.

    Usage:
        async with AsyncPyodideBox() as box:
            result = await box.run("1 + 1")
            print(result)  # 2
    """

    def __init__(
        self,
        *,
        allow_net: bool = True,
        allow_read: bool = True,
        ignore_cert_errors: bool = False
    ):
        """Initialize AsyncPyodideBox.

        Args:
            allow_net: Whether to allow network access (needed to download Pyodide).
            allow_read: Whether to allow file reading (needed for Pyodide WASM files).
            ignore_cert_errors: Whether to ignore certificate errors.
        """
        self._process: asyncio.subprocess.Process | None = None
        self._request_id = 0
        self._lock = asyncio.Lock()
        self._worker_path = Path(__file__).parent.parent / "pyodide_worker.js"
        self._allow_net = allow_net
        self._allow_read = allow_read
        self._ignore_cert_errors = ignore_cert_errors
        self._initialized = False

    async def start(self) -> None:
        """Start the Deno subprocess with Pyodide."""
        if self._process is not None:
            raise PyodideBoxError("AsyncPyodideBox already started")

        cmd = ["deno", "run"]
        if self._allow_net:
            cmd.append("--allow-net")
        if self._allow_read:
            cmd.append("--allow-read")
        if self._ignore_cert_errors:
            cmd.append("--unsafely-ignore-certificate-errors")
        cmd.append(str(self._worker_path))

        self._process = await asyncio.create_subprocess_exec(
            *cmd,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )

    async def stop(self) -> None:
        """Stop the Deno subprocess."""
        if self._process is None:
            return

        try:
            await self._send_request({"type": "shutdown"})
        except Exception:
            pass

        try:
            self._process.terminate()
            await asyncio.wait_for(self._process.wait(), timeout=5)
        except asyncio.TimeoutError:
            self._process.kill()
            await self._process.wait()

        self._process = None
        self._initialized = False

    async def __aenter__(self) -> "AsyncPyodideBox":
        await self.start()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        await self.stop()

    async def _send_request(self, request: dict) -> dict:
        """Send a request to the Deno worker and get response."""
        if self._process is None:
            raise PyodideBoxError("AsyncPyodideBox not started")

        async with self._lock:
            self._request_id += 1
            request["id"] = self._request_id

            request_line = json.dumps(request) + "\n"
            self._process.stdin.write(request_line.encode())
            await self._process.stdin.drain()

            response_line = await self._process.stdout.readline()
            if not response_line:
                stderr = await self._process.stderr.read()
                raise PyodideBoxError(f"Deno process terminated unexpectedly: {stderr.decode()}")

            return json.loads(response_line.decode())

    async def init(self) -> dict:
        """Initialize Pyodide (downloads and loads it).

        Returns:
            Dict with initialization info including Pyodide version.
        """
        response = await self._send_request({"type": "init"})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        self._initialized = True
        return response.get("result", {})

    async def run(self, code: str) -> Any:
        """Execute Python code and return the result.

        Args:
            code: Python code to execute.

        Returns:
            The result of the last expression.
        """
        response = await self._send_request({"type": "run_python", "code": code})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result")

    async def set_global(self, name: str, value: Any) -> None:
        """Set a global variable in the Pyodide namespace."""
        response = await self._send_request({"type": "set_global", "name": name, "value": value})
        if "error" in response:
            raise PyodideBoxError(response["error"])

    async def get_global(self, name: str) -> Any:
        """Get a global variable from the Pyodide namespace."""
        response = await self._send_request({"type": "get_global", "name": name})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result")

    async def install(self, *packages: str) -> dict:
        """Install Python packages using micropip."""
        response = await self._send_request({
            "type": "install_packages",
            "packages": list(packages)
        })
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result", {})

    async def run_js(self, code: str) -> Any:
        """Execute JavaScript code in the Deno runtime."""
        response = await self._send_request({"type": "run_js", "code": code})
        if "error" in response:
            raise PyodideBoxError(response["error"])
        return response.get("result")
