"""Synchronous V86box implementation."""

from __future__ import annotations

import json
import os
import subprocess
import threading
import time
from pathlib import Path
from typing import Any, TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Self

# Default URLs for downloading assets
DEFAULT_ASSETS = {
    "bios": "https://cdn.jsdelivr.net/gh/copy/v86@master/bios/seabios.bin",
    "vga_bios": "https://cdn.jsdelivr.net/gh/copy/v86@master/bios/vgabios.bin",
    "bzimage": "https://static.simonwillison.net/static/cors-allow/2026/buildroot-bzimage68.bin",
}


class V86boxError(Exception):
    """Error raised when v86 execution fails."""

    pass


class V86box:
    """Synchronous wrapper for executing bash commands in a v86 Linux sandbox.

    Usage:
        with V86box() as box:
            result = box.exec("echo hello")
            print(result)  # "hello"
    """

    def __init__(
        self,
        bios_path: str | Path | None = None,
        vga_bios_path: str | Path | None = None,
        bzimage_path: str | Path | None = None,
        wasm_path: str | Path | None = None,
        assets_dir: str | Path | None = None,
        boot_timeout: float = 60.0,
        command_timeout: float = 30.0,
    ):
        """Initialize V86box.

        Args:
            bios_path: Path to seabios.bin. If not provided, downloads to assets_dir.
            vga_bios_path: Path to vgabios.bin. If not provided, downloads to assets_dir.
            bzimage_path: Path to Linux bzimage. If not provided, downloads to assets_dir.
            wasm_path: Path to v86.wasm. If not provided, uses from npm package.
            assets_dir: Directory for downloaded assets. Defaults to ~/.v86box.
            boot_timeout: Timeout in seconds for VM boot.
            command_timeout: Default timeout for command execution.
        """
        self._process: subprocess.Popen | None = None
        self._request_id = 0
        self._lock = threading.Lock()
        self._worker_path = Path(__file__).parent / "worker.js"
        self._boot_timeout = boot_timeout
        self._command_timeout = command_timeout

        # Set up assets directory
        if assets_dir is None:
            assets_dir = Path.home() / ".v86box"
        self._assets_dir = Path(assets_dir)
        self._assets_dir.mkdir(parents=True, exist_ok=True)

        # Set paths
        self._bios_path = Path(bios_path) if bios_path else None
        self._vga_bios_path = Path(vga_bios_path) if vga_bios_path else None
        self._bzimage_path = Path(bzimage_path) if bzimage_path else None
        self._wasm_path = Path(wasm_path) if wasm_path else None

    def _ensure_asset(self, name: str, path: Path | None, url: str) -> Path:
        """Ensure an asset file exists, downloading if necessary."""
        if path and path.exists():
            return path

        # Use default location in assets dir
        default_path = self._assets_dir / Path(url).name
        if default_path.exists():
            return default_path

        # Download the asset
        import urllib.request

        print(f"Downloading {name}...")
        urllib.request.urlretrieve(url, default_path)
        return default_path

    def _ensure_wasm(self) -> Path:
        """Ensure v86.wasm exists."""
        if self._wasm_path and self._wasm_path.exists():
            return self._wasm_path

        # Check assets dir
        wasm_path = self._assets_dir / "v86.wasm"
        if wasm_path.exists():
            return wasm_path

        # Try to find in npm package (install if needed)
        try:
            import subprocess

            # Check if v86 is installed globally or try to find it
            result = subprocess.run(
                ["npm", "root", "-g"],
                capture_output=True,
                text=True,
            )
            global_root = Path(result.stdout.strip())
            npm_wasm = global_root / "v86" / "build" / "v86.wasm"
            if npm_wasm.exists():
                return npm_wasm
        except Exception:
            pass

        # Download from npm via unpkg
        print("Downloading v86.wasm from npm...")
        import urllib.request

        wasm_url = "https://unpkg.com/v86@0.5.301/build/v86.wasm"
        urllib.request.urlretrieve(wasm_url, wasm_path)
        return wasm_path

    def _ensure_v86_lib(self) -> Path:
        """Ensure v86 JavaScript library exists and return its path."""
        # Check assets dir for local copy
        lib_path = self._assets_dir / "libv86.mjs"
        if lib_path.exists():
            return lib_path

        # Try to find in npm package
        try:
            import subprocess

            # Check local node_modules first
            for node_modules in [
                Path("/tmp/v86-test/node_modules"),  # From our test setup
                Path.cwd() / "node_modules",
            ]:
                npm_lib = node_modules / "v86" / "build" / "libv86.mjs"
                if npm_lib.exists():
                    return npm_lib

            # Check global npm
            result = subprocess.run(
                ["npm", "root", "-g"],
                capture_output=True,
                text=True,
            )
            global_root = Path(result.stdout.strip())
            npm_lib = global_root / "v86" / "build" / "libv86.mjs"
            if npm_lib.exists():
                return npm_lib
        except Exception:
            pass

        # Download from npm via unpkg
        print("Downloading libv86.mjs from npm...")
        import urllib.request

        lib_url = "https://unpkg.com/v86@0.5.301/build/libv86.mjs"
        urllib.request.urlretrieve(lib_url, lib_path)
        return lib_path

    def _find_deno(self) -> str:
        """Find the deno binary."""
        # Try the deno Python package first
        try:
            from deno import find_deno_bin

            return find_deno_bin()
        except ImportError:
            pass

        # Try PATH
        import shutil

        deno = shutil.which("deno")
        if deno:
            return deno

        raise V86boxError(
            "Deno not found. Install the 'deno' package: pip install deno"
        )

    def start(self) -> None:
        """Start the v86 Linux VM."""
        if self._process is not None:
            raise V86boxError("V86box already started")

        # Ensure all assets exist
        bios = self._ensure_asset("BIOS", self._bios_path, DEFAULT_ASSETS["bios"])
        vga_bios = self._ensure_asset(
            "VGA BIOS", self._vga_bios_path, DEFAULT_ASSETS["vga_bios"]
        )
        bzimage = self._ensure_asset(
            "bzimage", self._bzimage_path, DEFAULT_ASSETS["bzimage"]
        )
        wasm = self._ensure_wasm()
        v86_lib = self._ensure_v86_lib()

        # Find deno
        deno = self._find_deno()

        # Set up environment
        env = os.environ.copy()
        env["V86_BIOS_PATH"] = str(bios)
        env["V86_VGA_BIOS_PATH"] = str(vga_bios)
        env["V86_BZIMAGE_PATH"] = str(bzimage)
        env["V86_WASM_PATH"] = str(wasm)
        env["V86_LIB_PATH"] = str(v86_lib)

        # Start Deno process
        self._process = subprocess.Popen(
            [deno, "run", "--allow-read", "--allow-env", str(self._worker_path)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            env=env,
        )

        # Wait for VM to boot
        self._wait_for_ready()

    def _wait_for_ready(self) -> None:
        """Wait for the VM to boot and become ready."""
        start_time = time.time()

        while time.time() - start_time < self._boot_timeout:
            line = self._process.stdout.readline()
            if not line:
                stderr = self._process.stderr.read()
                raise V86boxError(f"VM process terminated: {stderr}")

            try:
                response = json.loads(line)
                if response.get("type") == "ready":
                    return
                if "error" in response:
                    raise V86boxError(response["error"])
            except json.JSONDecodeError:
                continue

        raise V86boxError(f"VM failed to boot within {self._boot_timeout} seconds")

    def stop(self) -> None:
        """Stop the v86 Linux VM."""
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

    def __enter__(self) -> "V86box":
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.stop()

    def _send_request(self, request: dict, timeout: float | None = None) -> dict:
        """Send a request to the Deno worker and get response."""
        if self._process is None:
            raise V86boxError("V86box not started")

        if timeout is None:
            timeout = self._command_timeout

        with self._lock:
            self._request_id += 1
            request["id"] = self._request_id

            # Send request
            request_line = json.dumps(request) + "\n"
            self._process.stdin.write(request_line)
            self._process.stdin.flush()

            # Read response (may need to wait for command to complete)
            start_time = time.time()
            while time.time() - start_time < timeout:
                line = self._process.stdout.readline()
                if not line:
                    stderr = self._process.stderr.read()
                    raise V86boxError(f"VM process terminated: {stderr}")

                try:
                    response = json.loads(line)
                    if response.get("id") == self._request_id:
                        return response
                except json.JSONDecodeError:
                    continue

            raise V86boxError(f"Command timed out after {timeout} seconds")

    def exec(self, command: str, timeout: float | None = None) -> str:
        """Execute a bash command and return the output.

        Args:
            command: Bash command to execute.
            timeout: Timeout in seconds. Defaults to command_timeout from init.

        Returns:
            The stdout output of the command.

        Raises:
            V86boxError: If the command fails or times out.
        """
        response = self._send_request({"type": "exec", "command": command}, timeout)

        if "error" in response:
            raise V86boxError(response["error"])

        return response.get("result", "")

    def status(self) -> dict:
        """Get the status of the VM.

        Returns:
            A dict with status information including:
            - ready: Whether the VM is ready for commands
            - pending: Number of pending commands
        """
        response = self._send_request({"type": "status"})

        if "error" in response:
            raise V86boxError(response["error"])

        return response.get("result", {})
