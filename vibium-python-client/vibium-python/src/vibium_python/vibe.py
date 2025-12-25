"""Vibe class for browser automation."""

from typing import TYPE_CHECKING

from .element import BoundingBox, Element, ElementInfo

if TYPE_CHECKING:
    from .bidi import BiDiClient
    from .clicker import ClickerProcess


class Vibe:
    """Main browser automation interface."""

    def __init__(self, client: "BiDiClient", process: "ClickerProcess | None"):
        """Initialize the Vibe instance.

        Args:
            client: The BiDi client.
            process: The clicker process (if we started it).
        """
        self._client = client
        self._process = process
        self._context: str | None = None

    def _get_context(self) -> str:
        """Get the current browsing context ID."""
        if self._context:
            return self._context

        result = self._client.send("browsingContext.getTree", {})
        contexts = result.get("contexts", [])
        if not contexts:
            raise RuntimeError("No browsing context available")

        self._context = contexts[0]["context"]
        return self._context

    def go(self, url: str) -> None:
        """Navigate to a URL.

        Args:
            url: The URL to navigate to.
        """
        context = self._get_context()
        self._client.send(
            "browsingContext.navigate",
            {
                "context": context,
                "url": url,
                "wait": "complete",
            },
        )

    def screenshot(self) -> bytes:
        """Capture a screenshot of the viewport.

        Returns:
            PNG image data as bytes.
        """
        import base64

        context = self._get_context()
        result = self._client.send(
            "browsingContext.captureScreenshot",
            {"context": context},
        )
        return base64.b64decode(result["data"])

    def find(self, selector: str, timeout: int | None = None) -> Element:
        """Find an element by CSS selector.

        Waits for the element to exist before returning.

        Args:
            selector: CSS selector for the element.
            timeout: Optional timeout in milliseconds.

        Returns:
            An Element instance.
        """
        context = self._get_context()
        params = {
            "context": context,
            "selector": selector,
        }
        if timeout is not None:
            params["timeout"] = timeout

        result = self._client.send("vibium:find", params)

        box_data = result["box"]
        info = ElementInfo(
            tag=result["tag"],
            text=result["text"],
            box=BoundingBox(
                x=box_data["x"],
                y=box_data["y"],
                width=box_data["width"],
                height=box_data["height"],
            ),
        )

        return Element(self._client, context, selector, info)

    def evaluate(self, script: str) -> any:
        """Execute JavaScript in the page context.

        Args:
            script: JavaScript code to execute.

        Returns:
            The result of the script execution.
        """
        context = self._get_context()
        result = self._client.send(
            "script.callFunction",
            {
                "functionDeclaration": f"() => {{ {script} }}",
                "target": {"context": context},
                "arguments": [],
                "awaitPromise": True,
                "resultOwnership": "root",
            },
        )
        return result["result"].get("value")

    def quit(self) -> None:
        """Close the browser and clean up resources."""
        self._client.close()
        if self._process:
            self._process.stop()
