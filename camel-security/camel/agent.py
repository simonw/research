"""
Main CaMeL Agent that orchestrates the entire system.

This module provides:
- CaMeLAgent class for executing user queries safely
- ExecutionResult for returning detailed execution information
- Error handling with retry logic
"""

from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional, Tuple

from .config import CaMeLConfig, InterpreterMode
from .errors import (
    CaMeLException,
    InterpreterError,
    NotEnoughInformationError,
    SecurityPolicyViolation,
    ToolExecutionError,
)
from .interpreter import CaMeLInterpreter
from .llm import AnthropicPLLM, AnthropicQLLM, PrivilegedLLM, QuarantinedLLM
from .policies import (
    PolicyDecision,
    SecurityPolicy,
    SendEmailPolicy,
    CreateCalendarEventPolicy,
    SendMoneyPolicy,
    ShareFilePolicy,
)
from .tools import ToolRegistry, tool_registry, ToolSignature


@dataclass
class ExecutionResult:
    """
    Result of agent execution.

    Contains detailed information about the execution including
    success status, results, trace, and any errors encountered.
    """
    success: bool
    result: Any
    trace: List[Tuple[str, Dict[str, Any], Any]] = field(default_factory=list)
    policy_triggers: List[PolicyDecision] = field(default_factory=list)
    errors: List[Exception] = field(default_factory=list)
    generated_code: str = ""
    retry_count: int = 0


@dataclass
class ErrorContext:
    """Context for error recovery."""
    exception: Exception
    code: str
    line_number: Optional[int]
    executed_variables: Dict[str, Any]
    is_trusted: bool


class ErrorHandler:
    """Handles errors and prepares context for P-LLM retry."""

    def __init__(self, max_retries: int = 10):
        self._max_retries = max_retries
        self._retry_count = 0

    def can_retry(self) -> bool:
        """Check if more retries are allowed."""
        return self._retry_count < self._max_retries

    @property
    def retry_count(self) -> int:
        """Get the current retry count."""
        return self._retry_count

    def reset(self) -> None:
        """Reset the retry counter."""
        self._retry_count = 0

    def prepare_retry_context(self, error_ctx: ErrorContext) -> str:
        """
        Prepare error message for P-LLM retry.

        Args:
            error_ctx: Context about the error

        Returns:
            Formatted error message for P-LLM
        """
        self._retry_count += 1

        # Redact error message if it comes from untrusted source
        if not error_ctx.is_trusted:
            error_message = (
                "<The exception was redacted because it came from an "
                "untrusted source. Try to infer what the problem was "
                "from the context provided.>"
            )
        else:
            error_message = str(error_ctx.exception)

        return f"""
Running the code gave the following error:
Traceback (most recent call last):
  File "<stdin>", line {error_ctx.line_number or '?'}, in <module>
{type(error_ctx.exception).__name__}: {error_message}

Provide the new code with the error fixed. Provide *all the code* so that
I can directly run it. If the error comes from a search query that did not
return any results, then try the query with different parameters.

The code up to the line before the one where the exception was thrown has
already been executed and the variables and defined classes will still be
accessible to you. It's very important that you do not re-write code to
run functions that have side-effects (e.g., functions that send an email).
"""


class CaMeLAgent:
    """
    Main CaMeL agent class that orchestrates the entire system.

    The agent coordinates between:
    - P-LLM for code generation
    - Q-LLM for data parsing
    - Interpreter for code execution
    - Security policies for access control
    """

    def __init__(
        self,
        config: CaMeLConfig,
        p_llm: Optional[PrivilegedLLM] = None,
        q_llm: Optional[QuarantinedLLM] = None,
        custom_tools: Optional[Dict[str, Callable]] = None,
    ):
        """
        Initialize the CaMeL agent.

        Args:
            config: CaMeL configuration
            p_llm: Optional custom P-LLM (defaults to AnthropicPLLM)
            q_llm: Optional custom Q-LLM (defaults to AnthropicQLLM)
            custom_tools: Optional additional tools to register
        """
        self._config = config

        # Initialize LLMs
        self._p_llm = p_llm or self._create_p_llm(config)
        self._q_llm = q_llm or self._create_q_llm(config)

        # Load tools
        self._tools = self._load_tools(custom_tools)

        # Load policies
        self._policies = self._load_policies()

        # Initialize error handler
        self._error_handler = ErrorHandler(max_retries=config.max_retries)

    def _create_p_llm(self, config: CaMeLConfig) -> PrivilegedLLM:
        """Create the P-LLM instance."""
        return AnthropicPLLM(
            api_key=config.p_llm.api_key,
            model=config.p_llm.model_name,
            temperature=config.p_llm.temperature,
            max_tokens=config.p_llm.max_tokens,
        )

    def _create_q_llm(self, config: CaMeLConfig) -> QuarantinedLLM:
        """Create the Q-LLM instance."""
        return AnthropicQLLM(
            api_key=config.q_llm.api_key,
            model=config.q_llm.model_name,
            temperature=config.q_llm.temperature,
            max_tokens=config.q_llm.max_tokens,
        )

    def _load_tools(self, custom_tools: Optional[Dict[str, Callable]] = None) -> Dict[str, Callable]:
        """Load enabled tools."""
        tools = {}

        # Load from global registry
        for name in self._config.enabled_tools:
            tool_def = tool_registry.get(name)
            if tool_def:
                tools[name] = tool_def.function

        # Add custom tools
        if custom_tools:
            tools.update(custom_tools)

        return tools

    def _load_policies(self) -> List[SecurityPolicy]:
        """Load enabled security policies."""
        policy_map = {
            "send_email": SendEmailPolicy(),
            "create_calendar_event": CreateCalendarEventPolicy(),
            "send_money": SendMoneyPolicy(),
            "share_file": ShareFilePolicy(),
        }

        policies = []
        for name in self._config.enabled_policies:
            if name in policy_map:
                policies.append(policy_map[name])

        # Add custom policies
        policies.extend(self._config.custom_policies.values())

        return policies

    def _get_tool_signatures(self) -> List[ToolSignature]:
        """Get signatures for enabled tools."""
        signatures = []

        for name in self._tools:
            tool_def = tool_registry.get(name)
            if tool_def:
                signatures.append(ToolSignature(
                    name=tool_def.name,
                    description=tool_def.description,
                    parameters=tool_def.parameters,
                    return_type=tool_def.return_type.__name__ if hasattr(tool_def.return_type, '__name__') else str(tool_def.return_type),
                ))

        return signatures

    def _build_system_context(self) -> str:
        """Build system context string for P-LLM."""
        parts = []

        if self._config.user_name:
            parts.append(f"User Name: {self._config.user_name}")
        if self._config.user_email:
            parts.append(f"User Email: {self._config.user_email}")
        if self._config.company_name:
            parts.append(f"Company: {self._config.company_name}")

        # Add current date/time
        from datetime import datetime
        parts.append(f"Current Date: {datetime.now().strftime('%Y-%m-%d')}")
        parts.append(f"Current Time: {datetime.now().strftime('%H:%M')}")

        return "\n".join(parts)

    def execute(
        self,
        user_query: str,
        system_context: Optional[str] = None,
    ) -> ExecutionResult:
        """
        Execute a user query.

        Args:
            user_query: Natural language query from the user
            system_context: Optional additional context

        Returns:
            ExecutionResult with execution details
        """
        # Reset error handler
        self._error_handler.reset()

        errors: List[Exception] = []
        policy_triggers: List[PolicyDecision] = []
        generated_code = ""
        previous_code: Optional[str] = None
        error_message: Optional[str] = None

        # Build context
        full_context = self._build_system_context()
        if system_context:
            full_context += "\n" + system_context

        while self._error_handler.can_retry():
            try:
                # Generate code with P-LLM
                response = self._p_llm.generate_code(
                    user_query=user_query,
                    tools=self._get_tool_signatures(),
                    system_context=full_context,
                    previous_code=previous_code,
                    error_message=error_message,
                )

                generated_code = response.code
                previous_code = generated_code

                # Create interpreter for this execution
                interpreter = CaMeLInterpreter(
                    tools=self._tools,
                    security_policies=self._policies,
                    q_llm=self._q_llm,
                    mode=self._config.mode,
                    max_iterations=self._config.max_iterations,
                )

                # Execute with interpreter
                result, trace = interpreter.execute(generated_code)

                return ExecutionResult(
                    success=True,
                    result=result,
                    trace=trace,
                    policy_triggers=policy_triggers,
                    errors=errors,
                    generated_code=generated_code,
                    retry_count=self._error_handler.retry_count,
                )

            except SecurityPolicyViolation as e:
                policy_triggers.append(PolicyDecision(
                    result=e.policy if hasattr(e, 'policy') else None,
                    reason=str(e),
                ))

                if self._config.require_user_confirmation_on_deny:
                    # Return with policy violation - caller can handle confirmation
                    return ExecutionResult(
                        success=False,
                        result=None,
                        trace=[],
                        policy_triggers=policy_triggers,
                        errors=[e],
                        generated_code=generated_code,
                        retry_count=self._error_handler.retry_count,
                    )
                else:
                    errors.append(e)
                    error_ctx = ErrorContext(
                        exception=e,
                        code=generated_code,
                        line_number=None,
                        executed_variables={},
                        is_trusted=True,
                    )
                    error_message = self._error_handler.prepare_retry_context(error_ctx)

            except NotEnoughInformationError as e:
                errors.append(e)
                error_ctx = ErrorContext(
                    exception=e,
                    code=generated_code,
                    line_number=None,
                    executed_variables={},
                    is_trusted=False,  # Q-LLM output is untrusted
                )
                error_message = self._error_handler.prepare_retry_context(error_ctx)

            except InterpreterError as e:
                errors.append(e)
                error_ctx = ErrorContext(
                    exception=e,
                    code=generated_code,
                    line_number=e.line,
                    executed_variables={},
                    is_trusted=True,
                )
                error_message = self._error_handler.prepare_retry_context(error_ctx)

            except ToolExecutionError as e:
                errors.append(e)
                error_ctx = ErrorContext(
                    exception=e,
                    code=generated_code,
                    line_number=None,
                    executed_variables={},
                    is_trusted=e.is_trusted_error,
                )
                error_message = self._error_handler.prepare_retry_context(error_ctx)

            except Exception as e:
                # Unexpected error
                errors.append(e)
                error_ctx = ErrorContext(
                    exception=e,
                    code=generated_code,
                    line_number=None,
                    executed_variables={},
                    is_trusted=True,
                )
                error_message = self._error_handler.prepare_retry_context(error_ctx)

        # Max retries exceeded
        return ExecutionResult(
            success=False,
            result=None,
            trace=[],
            policy_triggers=policy_triggers,
            errors=errors,
            generated_code=generated_code,
            retry_count=self._error_handler.retry_count,
        )

    def execute_code(self, code: str) -> ExecutionResult:
        """
        Execute pre-written code directly (bypassing P-LLM).

        Useful for testing or when code is provided by other means.

        Args:
            code: Python code to execute

        Returns:
            ExecutionResult with execution details
        """
        try:
            interpreter = CaMeLInterpreter(
                tools=self._tools,
                security_policies=self._policies,
                q_llm=self._q_llm,
                mode=self._config.mode,
                max_iterations=self._config.max_iterations,
            )

            result, trace = interpreter.execute(code)

            return ExecutionResult(
                success=True,
                result=result,
                trace=trace,
                generated_code=code,
            )

        except Exception as e:
            return ExecutionResult(
                success=False,
                result=None,
                errors=[e],
                generated_code=code,
            )

    def register_tool(
        self,
        name: str,
        function: Callable,
        description: str,
    ) -> None:
        """
        Register a new tool at runtime.

        Args:
            name: Tool name
            function: Tool function
            description: Tool description
        """
        tool_registry.register_tool(name, function, description)
        self._tools[name] = function

    def add_policy(self, policy: SecurityPolicy) -> None:
        """
        Add a security policy at runtime.

        Args:
            policy: Security policy to add
        """
        self._policies.append(policy)
