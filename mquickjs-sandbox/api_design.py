"""
mquickjs-sandbox: Unified API Design

All implementations (FFI, C Extension, WASM+wasmer, WASM+wasmtime, etc.)
should conform to this interface.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any, Optional, Union


@dataclass
class SandboxResult:
    """Result from executing JavaScript code."""
    success: bool
    value: Any  # The return value (None, int, float, str, bool, list, dict)
    error: Optional[str] = None  # Error message if success=False


class SandboxError(Exception):
    """Exception raised when sandbox execution fails."""
    pass


class TimeoutError(SandboxError):
    """Execution exceeded time limit."""
    pass


class MemoryError(SandboxError):
    """Execution exceeded memory limit."""
    pass


class BaseSandbox(ABC):
    """
    Base class for mquickjs sandbox implementations.

    Default limits:
    - memory_limit_bytes: 1MB (1,048,576 bytes)
    - time_limit_ms: 1000ms (1 second)
    """

    DEFAULT_MEMORY_LIMIT = 1024 * 1024  # 1MB
    DEFAULT_TIME_LIMIT_MS = 1000  # 1 second

    def __init__(
        self,
        memory_limit_bytes: int = DEFAULT_MEMORY_LIMIT,
        time_limit_ms: int = DEFAULT_TIME_LIMIT_MS,
    ):
        """
        Initialize the sandbox.

        Args:
            memory_limit_bytes: Maximum memory the JS engine can use.
            time_limit_ms: Maximum execution time in milliseconds.
        """
        if memory_limit_bytes < 8192:
            raise ValueError("memory_limit_bytes must be at least 8192 (8KB)")
        if time_limit_ms < 0:
            raise ValueError("time_limit_ms must be non-negative (0 = no limit)")

        self.memory_limit_bytes = memory_limit_bytes
        self.time_limit_ms = time_limit_ms

    @abstractmethod
    def execute(self, code: str) -> Any:
        """
        Execute JavaScript code and return the result.

        Args:
            code: JavaScript source code to execute.

        Returns:
            The result of the JavaScript expression, converted to Python:
            - undefined/null -> None
            - boolean -> bool
            - number -> int or float
            - string -> str
            - array -> list (recursively converted)
            - object -> dict (recursively converted)

        Raises:
            SandboxError: For JavaScript runtime errors.
            TimeoutError: If execution exceeds time limit.
            MemoryError: If execution exceeds memory limit.
        """
        pass

    def execute_safe(self, code: str) -> SandboxResult:
        """
        Execute JavaScript code and return a SandboxResult.

        This never raises exceptions - errors are returned in the result.

        Args:
            code: JavaScript source code to execute.

        Returns:
            SandboxResult with success flag and value/error.
        """
        try:
            value = self.execute(code)
            return SandboxResult(success=True, value=value)
        except SandboxError as e:
            return SandboxResult(success=False, value=None, error=str(e))


def execute_js(
    code: str,
    *,
    memory_limit_bytes: int = BaseSandbox.DEFAULT_MEMORY_LIMIT,
    time_limit_ms: int = BaseSandbox.DEFAULT_TIME_LIMIT_MS,
) -> Any:
    """
    Execute JavaScript code in a sandbox.

    This is a convenience function that creates a temporary sandbox,
    executes the code, and returns the result.

    Args:
        code: JavaScript source code to execute.
        memory_limit_bytes: Maximum memory (default 1MB).
        time_limit_ms: Maximum execution time (default 1000ms).

    Returns:
        The result of the JavaScript expression.

    Raises:
        SandboxError: For JavaScript runtime errors.
        TimeoutError: If execution exceeds time limit.
        MemoryError: If execution exceeds memory limit.
    """
    # This will be implemented by each backend
    raise NotImplementedError("Use a specific implementation")
