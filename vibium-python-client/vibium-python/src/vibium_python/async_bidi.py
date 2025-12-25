"""Async WebDriver BiDi protocol client."""

import asyncio
import json
from typing import Any

import websockets


class AsyncBiDiClient:
    """Async WebDriver BiDi client using websockets."""

    def __init__(self, url: str):
        """Initialize the client.

        Args:
            url: WebSocket URL to connect to.
        """
        self.url = url
        self._ws = None
        self._next_id = 1
        self._lock = asyncio.Lock()

    async def connect(self):
        """Connect to the WebSocket server."""
        self._ws = await websockets.connect(self.url)

    async def close(self):
        """Close the WebSocket connection."""
        if self._ws:
            await self._ws.close()
            self._ws = None

    async def send(self, method: str, params: dict | None = None) -> Any:
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

        async with self._lock:
            command_id = self._next_id
            self._next_id += 1

        command = {
            "id": command_id,
            "method": method,
            "params": params or {},
        }

        await self._ws.send(json.dumps(command))

        # Wait for response with matching ID
        while True:
            raw_message = await self._ws.recv()
            message = json.loads(raw_message)

            # Check if this is the response we're waiting for
            if message.get("id") == command_id:
                if message.get("type") == "error" or message.get("error"):
                    error = message.get("error", {})
                    error_type = error.get("error", "unknown error")
                    error_msg = error.get("message", "unknown error")
                    raise RuntimeError(f"BiDi error: {error_type} - {error_msg}")
                return message.get("result")

    async def __aenter__(self):
        """Async context manager entry."""
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.close()
        return False
