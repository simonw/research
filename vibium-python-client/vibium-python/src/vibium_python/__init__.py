"""Vibium Python Client - Browser automation for AI agents and humans."""

from . import browser
from .bidi import BiDiClient
from .clicker import ClickerProcess
from .element import BoundingBox, Element, ElementInfo
from .vibe import Vibe

__all__ = [
    "browser",
    "BiDiClient",
    "ClickerProcess",
    "Element",
    "ElementInfo",
    "BoundingBox",
    "Vibe",
]

__version__ = "0.1.0"
