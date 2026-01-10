"""
CaMeL: Capabilities for Machine Learning

A defense system against prompt injection attacks in LLM-based agentic systems.

Based on "Defeating Prompt Injections by Design" (Debenedetti et al., 2025)
"""

from .types import (
    CaMeLValue,
    Capability,
    DataSource,
    Public,
    PublicSingleton,
    CapabilityAssigner,
)
from .errors import (
    CaMeLException,
    InterpreterError,
    SecurityPolicyViolation,
    NotEnoughInformationError,
    ToolExecutionError,
)
from .policies import (
    SecurityPolicy,
    SecurityPolicyResult,
    PolicyDecision,
    PolicyRegistry,
    PolicyBuilder,
    is_trusted,
    can_readers_read_value,
)
from .interpreter import (
    CaMeLInterpreter,
    InterpreterMode,
    InterpreterState,
    DataFlowGraph,
)
from .tools import (
    ToolDefinition,
    ToolSignature,
    ToolRegistry,
    tool_registry,
)
from .llm import (
    PrivilegedLLM,
    QuarantinedLLM,
    PLLMResponse,
    QLLMOutput,
)
from .agent import CaMeLAgent, ExecutionResult
from .config import CaMeLConfig, LLMConfig, load_config

__version__ = "1.0.0"
__all__ = [
    # Types
    "CaMeLValue",
    "Capability",
    "DataSource",
    "Public",
    "PublicSingleton",
    "CapabilityAssigner",
    # Errors
    "CaMeLException",
    "InterpreterError",
    "SecurityPolicyViolation",
    "NotEnoughInformationError",
    "ToolExecutionError",
    # Policies
    "SecurityPolicy",
    "SecurityPolicyResult",
    "PolicyDecision",
    "PolicyRegistry",
    "PolicyBuilder",
    "is_trusted",
    "can_readers_read_value",
    # Interpreter
    "CaMeLInterpreter",
    "InterpreterMode",
    "InterpreterState",
    "DataFlowGraph",
    # Tools
    "ToolDefinition",
    "ToolSignature",
    "ToolRegistry",
    "tool_registry",
    # LLM
    "PrivilegedLLM",
    "QuarantinedLLM",
    "PLLMResponse",
    "QLLMOutput",
    # Agent
    "CaMeLAgent",
    "ExecutionResult",
    # Config
    "CaMeLConfig",
    "LLMConfig",
    "load_config",
]
