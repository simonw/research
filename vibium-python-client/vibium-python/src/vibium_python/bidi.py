"""WebDriver BiDi protocol client."""

import asyncio
import json
import threading
from dataclasses import dataclass
from typing import Any

import websockets
from websockets.sync.client import connect


@dataclass
class BiDiResponse:
    """Represents a BiDi response."""

    id: int
    type: str  # "success" or "error"
    result: Any = None
    error: dict | None = None


class BiDiClient:
    """Synchronous WebDriver BiDi client."""

    def __init__(self, url: str):
        """Initialize the client.

        Args:
            url: WebSocket URL to connect to.
        """
        self.url = url
        self._ws = None
        self._next_id = 1
        self._lock = threading.Lock()

    def connect(self):
        """Connect to the WebSocket server."""
        self._ws = connect(self.url)

    def close(self):
        """Close the WebSocket connection."""
        if self._ws:
            self._ws.close()
            self._ws = None

    def send(self, method: str, params: dict | None = None) -> Any:
        """Send a BiDi command and wait for the response.

        Args:
            method: The BiDi method to call.
            params: Optional parameters for the method.

        Returns:
            The result from the response.

        Raises:
            RuntimeError: If the response indicates an error.
        """
        if not self._ws:
            raise RuntimeError("Not connected")

        with self._lock:
            command_id = self._next_id
            self._next_id += 1

        command = {
            "id": command_id,
            "method": method,
            "params": params or {},
        }

        self._ws.send(json.dumps(command))

        # Wait for response with matching ID
        while True:
            raw_message = self._ws.recv()
            message = json.loads(raw_message)

            # Check if this is the response we're waiting for
            if message.get("id") == command_id:
                if message.get("type") == "error" or message.get("error"):
                    error = message.get("error", {})
                    error_type = error.get("error", "unknown error")
                    error_msg = error.get("message", "unknown error")
                    raise RuntimeError(f"BiDi error: {error_type} - {error_msg}")
                return message.get("result")

    def __enter__(self):
        """Context manager entry."""
        self.connect()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self.close()
        return False
