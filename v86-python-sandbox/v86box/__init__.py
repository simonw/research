"""v86box - Execute bash commands in a v86 Linux sandbox."""

from .sync_box import V86box, V86boxError

__all__ = ["V86box", "V86boxError"]
__version__ = "0.1.0"
