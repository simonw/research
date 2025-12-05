"""
Base classes and exceptions for jq WASM runners
"""

from abc import ABC, abstractmethod
from typing import Optional, Any
from dataclasses import dataclass


class JqError(Exception):
    """Base exception for jq execution errors"""
    pass


class JqTimeoutError(JqError):
    """Raised when jq execution exceeds CPU/instruction limits"""
    pass


class JqMemoryError(JqError):
    """Raised when jq execution exceeds memory limits"""
    pass


class JqSyntaxError(JqError):
    """Raised when jq program has syntax errors"""
    pass


@dataclass
class JqResult:
    """Result from jq execution"""
    stdout: str
    stderr: str
    exit_code: int

    @property
    def success(self) -> bool:
        return self.exit_code == 0

    @property
    def output(self) -> str:
        """Return stdout if successful, raise error otherwise"""
        if self.success:
            return self.stdout.strip()
        raise JqError(f"jq failed (exit {self.exit_code}): {self.stderr}")


class JqRunner(ABC):
    """
    Abstract base class for jq WASM runners.

    Implementations must provide:
    - run(): Execute a jq program on JSON input
    - Memory and CPU limiting capabilities
    - Sandbox isolation (no filesystem access)
    """

    def __init__(
        self,
        wasm_path: str,
        max_memory_pages: int = 256,  # 256 pages = 16MB
        max_fuel: Optional[int] = 100_000_000,  # CPU instruction limit
    ):
        """
        Initialize the jq runner.

        Args:
            wasm_path: Path to the jq/jaq WASM binary
            max_memory_pages: Maximum memory in 64KB pages (default 16MB)
            max_fuel: Maximum fuel/instructions (None for unlimited)
        """
        self.wasm_path = wasm_path
        self.max_memory_pages = max_memory_pages
        self.max_fuel = max_fuel
        self._initialized = False

    @abstractmethod
    def _initialize(self) -> None:
        """Initialize the WASM runtime and load the module"""
        pass

    @abstractmethod
    def run(
        self,
        program: str,
        input_json: str,
        *,
        raw_output: bool = False,
        compact: bool = False,
    ) -> JqResult:
        """
        Execute a jq program on JSON input.

        Args:
            program: The jq program/filter to execute
            input_json: JSON input string
            raw_output: If True, output raw strings without JSON encoding
            compact: If True, produce compact output

        Returns:
            JqResult with stdout, stderr, and exit code

        Raises:
            JqTimeoutError: If execution exceeds fuel limit
            JqMemoryError: If execution exceeds memory limit
            JqError: For other execution errors
        """
        pass

    def query(self, program: str, input_json: str, **kwargs) -> str:
        """
        Convenience method that returns just the output string.

        Raises JqError on failure.
        """
        result = self.run(program, input_json, **kwargs)
        return result.output

    def __enter__(self):
        if not self._initialized:
            self._initialize()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        pass
