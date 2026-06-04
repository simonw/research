"""Clicker process management."""

import re
import subprocess
import time
from dataclasses import dataclass


@dataclass
class ClickerProcess:
    """Represents a running clicker process."""

    process: subprocess.Popen
    port: int

    @classmethod
    def start(
        cls,
        executable_path: str = "clicker",
        port: int = 0,
        headless: bool = False,
    ) -> "ClickerProcess":
        """Start the clicker serve process.

        Args:
            executable_path: Path to the clicker binary.
            port: Port to listen on. 0 means auto-select.
            headless: Whether to run browser in headless mode.

        Returns:
            A ClickerProcess instance.
        """
        args = [executable_path, "serve"]
        if port > 0:
            args.extend(["--port", str(port)])
        if headless:
            args.append("--headless")

        process = subprocess.Popen(
            args,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )

        # Wait for the server to start and extract the port
        actual_port = cls._wait_for_server(process)

        return cls(process=process, port=actual_port)

    @staticmethod
    def _wait_for_server(process: subprocess.Popen, timeout: float = 10.0) -> int:
        """Wait for the server to start and return the port."""
        start_time = time.time()
        output = ""

        while time.time() - start_time < timeout:
            if process.poll() is not None:
                # Process exited
                raise RuntimeError(
                    f"clicker process exited unexpectedly: {output}"
                )

            line = process.stdout.readline()
            if line:
                output += line
                # Look for "Server listening on ws://localhost:PORT"
                match = re.search(r"Server listening on ws://localhost:(\d+)", output)
                if match:
                    return int(match.group(1))

        raise TimeoutError(
            f"Timeout waiting for clicker to start. Output: {output}"
        )

    def stop(self):
        """Stop the clicker process."""
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
