"""Browser launch module."""

from .bidi import BiDiClient
from .clicker import ClickerProcess
from .vibe import Vibe


def launch(
    headless: bool = False,
    port: int = 0,
    executable_path: str = "clicker",
) -> Vibe:
    """Launch a browser and return a Vibe instance.

    Args:
        headless: Whether to run the browser in headless mode.
        port: Port for the clicker server (0 = auto-select).
        executable_path: Path to the clicker binary.

    Returns:
        A Vibe instance for browser automation.
    """
    # Start the clicker process
    process = ClickerProcess.start(
        executable_path=executable_path,
        port=port,
        headless=headless,
    )

    # Connect to the proxy
    client = BiDiClient(f"ws://localhost:{process.port}")
    client.connect()

    return Vibe(client, process)
