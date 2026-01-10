"""Asynchronous DenoBox implementation."""

from __future__ import annotations

import asyncio
import base64
import json
from pathlib import Path
from typing import Any, TYPE_CHECKING

from .sync_box import DenoBoxError

if TYPE_CHECKING:
    from typing import Self


class AsyncWasmModule:
    """Async wrapper for a loaded WebAssembly module."""

    def __init__(self, box: AsyncDenoBox, module_id: str, exports: dict[str, str]):
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

    async def call(self, func: str, *args: Any) -> Any:
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

        response = await self._box._send_request(
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

    async def unload(self) -> None:
        """Unload the WASM module."""
        if self._unloaded:
            return

        response = await self._box._send_request(
            {
                "type": "unload_wasm",
                "moduleId": self._module_id,
            }
        )

        if "error" in response:
            raise DenoBoxError(response["error"])

        self._unloaded = True


class AsyncDenoBox:
    """Asynchronous wrapper for executing JavaScript in a Deno sandbox.

    Usage:
        async with AsyncDenoBox() as box:
            result = await box.eval("1 + 1")
            print(result)  # 2
    """

    def __init__(self):
        self._process: asyncio.subprocess.Process | None = None
        self._request_id = 0
        self._lock = asyncio.Lock()
        self._pending_requests: dict[int, asyncio.Future] = {}
        self._reader_task: asyncio.Task | None = None
        self._worker_path = Path(__file__).parent / "worker.js"

    async def start(self) -> None:
        """Start the Deno subprocess."""
        if self._process is not None:
            raise DenoBoxError("AsyncDenoBox already started")

        # Run Deno with no permissions - fully sandboxed
        # The worker communicates via stdin/stdout only
        self._process = await asyncio.create_subprocess_exec(
            "deno",
            "run",
            str(self._worker_path),
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )

        # Start background reader task
        self._reader_task = asyncio.create_task(self._read_responses())

    async def _read_responses(self) -> None:
        """Background task to read responses from the Deno process."""
        while self._process and self._process.stdout:
            try:
                line = await self._process.stdout.readline()
                if not line:
                    break

                response = json.loads(line.decode())
                request_id = response.get("id")

                if request_id in self._pending_requests:
                    future = self._pending_requests.pop(request_id)
                    if not future.done():
                        future.set_result(response)
            except asyncio.CancelledError:
                break
            except Exception:
                break

    async def stop(self) -> None:
        """Stop the Deno subprocess."""
        if self._process is None:
            return

        # Cancel reader task
        if self._reader_task:
            self._reader_task.cancel()
            try:
                await self._reader_task
            except asyncio.CancelledError:
                pass
            self._reader_task = None

        # Cancel pending requests
        for future in self._pending_requests.values():
            if not future.done():
                future.set_exception(DenoBoxError("DenoBox stopped"))
        self._pending_requests.clear()

        try:
            # Send shutdown command
            await self._send_request_raw({"type": "shutdown"})
        except Exception:
            pass

        try:
            self._process.terminate()
            await asyncio.wait_for(self._process.wait(), timeout=5)
        except asyncio.TimeoutError:
            self._process.kill()
            await self._process.wait()

        self._process = None

    async def __aenter__(self) -> "AsyncDenoBox":
        await self.start()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        await self.stop()

    async def _send_request_raw(self, request: dict) -> None:
        """Send a request without waiting for response."""
        if self._process is None or self._process.stdin is None:
            raise DenoBoxError("AsyncDenoBox not started")

        request_line = json.dumps(request) + "\n"
        self._process.stdin.write(request_line.encode())
        await self._process.stdin.drain()

    async def _send_request(self, request: dict) -> dict:
        """Send a request to the Deno worker and get response."""
        if self._process is None:
            raise DenoBoxError("AsyncDenoBox not started")

        async with self._lock:
            self._request_id += 1
            request_id = self._request_id
            request["id"] = request_id

            # Create future for this request
            future: asyncio.Future = asyncio.get_event_loop().create_future()
            self._pending_requests[request_id] = future

            # Send request
            await self._send_request_raw(request)

        # Wait for response (outside lock to allow concurrent requests)
        try:
            return await future
        except asyncio.CancelledError:
            self._pending_requests.pop(request_id, None)
            raise

    async def eval(self, code: str) -> Any:
        """Evaluate JavaScript code and return the result.

        Args:
            code: JavaScript code to evaluate.

        Returns:
            The result of the evaluation, converted from JSON.

        Raises:
            DenoBoxError: If the JavaScript code raises an error.
        """
        response = await self._send_request({"type": "eval", "code": code})

        if "error" in response:
            raise DenoBoxError(response["error"])

        return response.get("result")

    async def load_wasm(
        self, path: str | None = None, wasm_bytes: bytes | None = None
    ) -> AsyncWasmModule:
        """Load a WebAssembly module.

        Args:
            path: Path to the WASM file (read by Python, not Deno).
            wasm_bytes: Raw WASM bytes to load directly.

        Returns:
            An AsyncWasmModule wrapper for calling exported functions.

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

        response = await self._send_request({"type": "load_wasm", "bytes": bytes_b64})

        if "error" in response:
            raise DenoBoxError(response["error"])

        result = response.get("result", {})
        return AsyncWasmModule(
            box=self,
            module_id=result["moduleId"],
            exports=result["exports"],
        )
