"""Vibium Python Client - Browser automation for AI agents and humans."""

from . import async_browser, browser
from .async_bidi import AsyncBiDiClient
from .async_element import AsyncElement
from .async_vibe import AsyncVibe
from .bidi import BiDiClient
from .clicker import ClickerProcess
from .element import BoundingBox, Element, ElementInfo
from .vibe import Vibe

__all__ = [
    # Sync API
    "browser",
    "BiDiClient",
    "ClickerProcess",
    "Element",
    "ElementInfo",
    "BoundingBox",
    "Vibe",
    # Async API
    "async_browser",
    "AsyncBiDiClient",
    "AsyncElement",
    "AsyncVibe",
]

__version__ = "0.1.0"
