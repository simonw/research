"""Async browser launch module."""

from .async_bidi import AsyncBiDiClient
from .async_vibe import AsyncVibe
from .clicker import ClickerProcess


async def launch(
    headless: bool = False,
    port: int = 0,
    executable_path: str = "clicker",
) -> AsyncVibe:
    """Launch a browser and return an AsyncVibe instance.

    Args:
        headless: Whether to run the browser in headless mode.
        port: Port for the clicker server (0 = auto-select).
        executable_path: Path to the clicker binary.

    Returns:
        An AsyncVibe instance for async browser automation.
    """
    # Start the clicker process (sync operation)
    process = ClickerProcess.start(
        executable_path=executable_path,
        port=port,
        headless=headless,
    )

    # Connect to the proxy asynchronously
    client = AsyncBiDiClient(f"ws://localhost:{process.port}")
    await client.connect()

    return AsyncVibe(client, process)
