"""
Exception classes for the CaMeL system.
"""

from typing import Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from .policies import SecurityPolicy


class CaMeLException(Exception):
    """Base exception for CaMeL system."""
    pass


class InterpreterError(CaMeLException):
    """Error during code interpretation."""

    def __init__(self, message: str, line: Optional[int] = None):
        self.line = line
        super().__init__(f"Line {line}: {message}" if line else message)


class SecurityPolicyViolation(CaMeLException):
    """Security policy was violated."""

    def __init__(
        self,
        message: str,
        policy: Optional["SecurityPolicy"] = None,
        tool_name: Optional[str] = None,
    ):
        self.policy = policy
        self.tool_name = tool_name
        super().__init__(message)


class NotEnoughInformationError(CaMeLException):
    """Q-LLM could not extract required information."""
    pass


class ToolExecutionError(CaMeLException):
    """Error during tool execution."""

    def __init__(
        self,
        message: str,
        tool_name: str,
        is_trusted_error: bool = True,
    ):
        self.tool_name = tool_name
        self.is_trusted_error = is_trusted_error
        super().__init__(message)
