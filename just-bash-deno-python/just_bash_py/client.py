"""
Sync and async Python clients for the just-bash JSONL server.
"""

from __future__ import annotations

import asyncio
import json
import os
import shutil
import subprocess
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


# Find the server script relative to this file
_SERVER_SCRIPT = str(Path(__file__).parent.parent / "just_bash_server.ts")


def _find_deno() -> str:
    """Find the deno executable."""
    # Check common locations
    deno = shutil.which("deno")
    if deno:
        return deno
    # Check ~/.deno/bin/deno
    home_deno = os.path.expanduser("~/.deno/bin/deno")
    if os.path.isfile(home_deno):
        return home_deno
    # Check /root/.deno/bin/deno
    root_deno = "/root/.deno/bin/deno"
    if os.path.isfile(root_deno):
        return root_deno
    raise FileNotFoundError(
        "Could not find deno executable. Install it from https://deno.land"
    )


@dataclass(frozen=True)
class BashResult:
    """Result of executing a bash command."""

    stdout: str
    stderr: str
    exit_code: int
    error: Optional[str] = None

    @property
    def ok(self) -> bool:
        """True if the command exited with code 0."""
        return self.exit_code == 0


class JustBash:
    """
    Synchronous Python client for the just-bash JSONL server.

    Spawns a Deno process running just-bash and communicates via
    newline-delimited JSON over stdin/stdout.

    The bash environment persists across commands - files written in
    one command can be read in subsequent commands.

    Args:
        network: Enable network access (curl). Default False.
        deno_path: Path to deno executable. Auto-detected if not provided.
        server_script: Path to the JSONL server TypeScript file.
        ready_timeout: Seconds to wait for server startup. Default 30.
    """

    def __init__(
        self,
        *,
        network: bool = False,
        deno_path: Optional[str] = None,
        server_script: Optional[str] = None,
        ready_timeout: float = 30.0,
    ):
        self._network = network
        self._deno_path = deno_path or _find_deno()
        self._server_script = server_script or _SERVER_SCRIPT
        self._ready_timeout = ready_timeout
        self._process: Optional[subprocess.Popen] = None

    def start(self) -> None:
        """Start the just-bash server process."""
        if self._process is not None:
            raise RuntimeError("Server already started")

        args = [self._deno_path, "run", "--allow-all", self._server_script]
        if self._network:
            args.append("--network")

        env = os.environ.copy()
        env["DENO_TLS_CA_STORE"] = "system"

        self._process = subprocess.Popen(
            args,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
        )

        # Wait for READY signal on stderr
        self._wait_for_ready()

    def _wait_for_ready(self) -> None:
        """Wait for the server to signal it's ready on stderr."""
        import select
        import time

        assert self._process is not None
        assert self._process.stderr is not None

        deadline = time.monotonic() + self._ready_timeout
        buf = b""

        while time.monotonic() < deadline:
            # Use select to avoid blocking forever
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                break

            ready, _, _ = select.select(
                [self._process.stderr], [], [], min(remaining, 0.1)
            )
            if ready:
                chunk = self._process.stderr.read1(4096)  # type: ignore[union-attr]
                if not chunk:
                    break
                buf += chunk
                if b"READY" in buf:
                    return

        raise TimeoutError(
            f"Server did not become ready within {self._ready_timeout}s. "
            f"stderr so far: {buf.decode(errors='replace')}"
        )

    def stop(self) -> None:
        """Stop the just-bash server process."""
        if self._process is None:
            return

        try:
            # Try graceful shutdown
            self._send_raw({"id": str(uuid.uuid4()), "command": "__shutdown"})
            self._process.wait(timeout=5)
        except Exception:
            # Force kill if graceful shutdown fails
            self._process.kill()
            self._process.wait(timeout=5)
        finally:
            self._process = None

    def __enter__(self) -> JustBash:
        self.start()
        return self

    def __exit__(self, *args) -> None:
        self.stop()

    def _send_raw(self, request: dict) -> None:
        """Send a JSON request to the server."""
        assert self._process is not None
        assert self._process.stdin is not None
        line = json.dumps(request) + "\n"
        self._process.stdin.write(line.encode())
        self._process.stdin.flush()

    def _read_response(self) -> dict:
        """Read a JSON response from the server."""
        assert self._process is not None
        assert self._process.stdout is not None
        line = self._process.stdout.readline()
        if not line:
            raise ConnectionError("Server process closed stdout")
        return json.loads(line)

    def run(
        self,
        command: str,
        *,
        env: Optional[dict[str, str]] = None,
        cwd: Optional[str] = None,
        timeout_ms: Optional[int] = None,
    ) -> BashResult:
        """
        Execute a bash command and return the result.

        Args:
            command: The bash command to execute.
            env: Optional environment variables for this command.
            cwd: Optional working directory for this command.
            timeout_ms: Optional timeout in milliseconds.

        Returns:
            BashResult with stdout, stderr, exit_code, and optional error.
        """
        if self._process is None:
            raise RuntimeError("Server not started. Call start() or use as context manager.")

        request: dict = {
            "id": str(uuid.uuid4()),
            "command": command,
        }
        if env is not None:
            request["env"] = env
        if cwd is not None:
            request["cwd"] = cwd
        if timeout_ms is not None:
            request["timeout_ms"] = timeout_ms

        self._send_raw(request)
        resp = self._read_response()

        return BashResult(
            stdout=resp.get("stdout", ""),
            stderr=resp.get("stderr", ""),
            exit_code=resp.get("exit_code", 1),
            error=resp.get("error"),
        )

    def ping(self) -> bool:
        """Check if the server is responsive."""
        result = self.run("__ping")
        return result.stdout == "pong"

    def reset(self) -> None:
        """Reset the bash environment (clears all virtual files and state)."""
        self.run("__reset")

    def write_file(self, path: str, content: str) -> BashResult:
        """Write content to a virtual file."""
        # Use heredoc to safely handle arbitrary content
        safe_content = content.replace("\\", "\\\\").replace("'", "'\\''")
        return self.run(f"cat > {path} << 'JUSTBASHEOF'\n{content}\nJUSTBASHEOF")

    def read_file(self, path: str) -> str:
        """Read content from a virtual file."""
        result = self.run(f"cat {path}")
        if not result.ok:
            raise FileNotFoundError(f"File not found: {path}")
        return result.stdout


class AsyncJustBash:
    """
    Asynchronous Python client for the just-bash JSONL server.

    Same functionality as JustBash but with async/await API.

    Args:
        network: Enable network access (curl). Default False.
        deno_path: Path to deno executable. Auto-detected if not provided.
        server_script: Path to the JSONL server TypeScript file.
        ready_timeout: Seconds to wait for server startup. Default 30.
    """

    def __init__(
        self,
        *,
        network: bool = False,
        deno_path: Optional[str] = None,
        server_script: Optional[str] = None,
        ready_timeout: float = 30.0,
    ):
        self._network = network
        self._deno_path = deno_path or _find_deno()
        self._server_script = server_script or _SERVER_SCRIPT
        self._ready_timeout = ready_timeout
        self._process: Optional[asyncio.subprocess.Process] = None

    async def start(self) -> None:
        """Start the just-bash server process."""
        if self._process is not None:
            raise RuntimeError("Server already started")

        args = [self._deno_path, "run", "--allow-all", self._server_script]
        if self._network:
            args.append("--network")

        env = os.environ.copy()
        env["DENO_TLS_CA_STORE"] = "system"

        self._process = await asyncio.create_subprocess_exec(
            *args,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            env=env,
        )

        await self._wait_for_ready()

    async def _wait_for_ready(self) -> None:
        """Wait for the server to signal it's ready on stderr."""
        assert self._process is not None
        assert self._process.stderr is not None

        buf = b""
        try:
            while True:
                chunk = await asyncio.wait_for(
                    self._process.stderr.read(4096),
                    timeout=self._ready_timeout,
                )
                if not chunk:
                    break
                buf += chunk
                if b"READY" in buf:
                    return
        except asyncio.TimeoutError:
            raise TimeoutError(
                f"Server did not become ready within {self._ready_timeout}s. "
                f"stderr so far: {buf.decode(errors='replace')}"
            )

    async def stop(self) -> None:
        """Stop the just-bash server process."""
        if self._process is None:
            return

        try:
            await self._send_raw({"id": str(uuid.uuid4()), "command": "__shutdown"})
            await asyncio.wait_for(self._process.wait(), timeout=5)
        except Exception:
            self._process.kill()
            await asyncio.wait_for(self._process.wait(), timeout=5)
        finally:
            self._process = None

    async def __aenter__(self) -> AsyncJustBash:
        await self.start()
        return self

    async def __aexit__(self, *args) -> None:
        await self.stop()

    async def _send_raw(self, request: dict) -> None:
        """Send a JSON request to the server."""
        assert self._process is not None
        assert self._process.stdin is not None
        line = json.dumps(request) + "\n"
        self._process.stdin.write(line.encode())
        await self._process.stdin.drain()

    async def _read_response(self) -> dict:
        """Read a JSON response from the server."""
        assert self._process is not None
        assert self._process.stdout is not None
        line = await self._process.stdout.readline()
        if not line:
            raise ConnectionError("Server process closed stdout")
        return json.loads(line)

    async def run(
        self,
        command: str,
        *,
        env: Optional[dict[str, str]] = None,
        cwd: Optional[str] = None,
        timeout_ms: Optional[int] = None,
    ) -> BashResult:
        """
        Execute a bash command and return the result.

        Args:
            command: The bash command to execute.
            env: Optional environment variables for this command.
            cwd: Optional working directory for this command.
            timeout_ms: Optional timeout in milliseconds.

        Returns:
            BashResult with stdout, stderr, exit_code, and optional error.
        """
        if self._process is None:
            raise RuntimeError("Server not started. Call start() or use as context manager.")

        request: dict = {
            "id": str(uuid.uuid4()),
            "command": command,
        }
        if env is not None:
            request["env"] = env
        if cwd is not None:
            request["cwd"] = cwd
        if timeout_ms is not None:
            request["timeout_ms"] = timeout_ms

        await self._send_raw(request)
        resp = await self._read_response()

        return BashResult(
            stdout=resp.get("stdout", ""),
            stderr=resp.get("stderr", ""),
            exit_code=resp.get("exit_code", 1),
            error=resp.get("error"),
        )

    async def ping(self) -> bool:
        """Check if the server is responsive."""
        result = await self.run("__ping")
        return result.stdout == "pong"

    async def reset(self) -> None:
        """Reset the bash environment (clears all virtual files and state)."""
        await self.run("__reset")

    async def write_file(self, path: str, content: str) -> BashResult:
        """Write content to a virtual file."""
        return await self.run(f"cat > {path} << 'JUSTBASHEOF'\n{content}\nJUSTBASHEOF")

    async def read_file(self, path: str) -> str:
        """Read content from a virtual file."""
        result = await self.run(f"cat {path}")
        if not result.ok:
            raise FileNotFoundError(f"File not found: {path}")
        return result.stdout
