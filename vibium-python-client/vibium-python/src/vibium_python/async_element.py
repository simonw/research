"""Async Element class for interacting with page elements."""

from dataclasses import dataclass
from typing import TYPE_CHECKING

from .element import BoundingBox, ElementInfo

if TYPE_CHECKING:
    from .async_bidi import AsyncBiDiClient


class AsyncElement:
    """Represents a DOM element on the page (async version)."""

    def __init__(
        self,
        client: "AsyncBiDiClient",
        context: str,
        selector: str,
        info: ElementInfo,
    ):
        """Initialize the element.

        Args:
            client: The async BiDi client.
            context: The browsing context ID.
            selector: The CSS selector used to find this element.
            info: Information about the element.
        """
        self._client = client
        self._context = context
        self._selector = selector
        self.info = info

    @property
    def tag(self) -> str:
        """Get the element's tag name."""
        return self.info.tag

    @property
    def text(self) -> str:
        """Get the element's text content."""
        return self.info.text

    @property
    def bounding_box(self) -> BoundingBox:
        """Get the element's bounding box."""
        return self.info.box

    async def click(self, timeout: int | None = None) -> None:
        """Click the element.

        Waits for element to be visible, stable, receive events, and enabled.

        Args:
            timeout: Optional timeout in milliseconds.
        """
        params = {
            "context": self._context,
            "selector": self._selector,
        }
        if timeout is not None:
            params["timeout"] = timeout

        await self._client.send("vibium:click", params)

    async def type(self, text: str, timeout: int | None = None) -> None:
        """Type text into the element.

        Waits for element to be visible, stable, receive events, enabled, and editable.

        Args:
            text: The text to type.
            timeout: Optional timeout in milliseconds.
        """
        params = {
            "context": self._context,
            "selector": self._selector,
            "text": text,
        }
        if timeout is not None:
            params["timeout"] = timeout

        await self._client.send("vibium:type", params)

    async def get_text(self) -> str:
        """Get the current text content of the element.

        Returns:
            The element's text content.
        """
        result = await self._client.send(
            "script.callFunction",
            {
                "functionDeclaration": """(selector) => {
                    const el = document.querySelector(selector);
                    return el ? (el.textContent || '').trim() : null;
                }""",
                "target": {"context": self._context},
                "arguments": [{"type": "string", "value": self._selector}],
                "awaitPromise": False,
                "resultOwnership": "root",
            },
        )
        if result["result"]["type"] == "null":
            raise ValueError(f"Element not found: {self._selector}")
        return result["result"]["value"]

    async def get_attribute(self, name: str) -> str | None:
        """Get an attribute value from the element.

        Args:
            name: The attribute name.

        Returns:
            The attribute value, or None if not present.
        """
        result = await self._client.send(
            "script.callFunction",
            {
                "functionDeclaration": """(selector, attrName) => {
                    const el = document.querySelector(selector);
                    return el ? el.getAttribute(attrName) : null;
                }""",
                "target": {"context": self._context},
                "arguments": [
                    {"type": "string", "value": self._selector},
                    {"type": "string", "value": name},
                ],
                "awaitPromise": False,
                "resultOwnership": "root",
            },
        )
        if result["result"]["type"] == "null":
            return None
        return result["result"]["value"]
