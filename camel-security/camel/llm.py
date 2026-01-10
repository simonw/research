"""
LLM interfaces for the CaMeL system.

This module provides:
- Abstract base classes for P-LLM and Q-LLM
- Concrete implementations using the Anthropic API
- System prompts and response handling
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Type, TypeVar

from pydantic import BaseModel

from .errors import NotEnoughInformationError
from .tools import ToolSignature

T = TypeVar("T", bound=BaseModel)


@dataclass
class PLLMResponse:
    """Response from the P-LLM."""
    code: str  # Python code
    explanation: Optional[str] = None


class QLLMOutput(BaseModel):
    """Base class for Q-LLM outputs with information sufficiency flag."""
    have_enough_information: bool = True


class PrivilegedLLM(ABC):
    """
    Abstract base class for the Privileged LLM.

    The P-LLM receives ONLY the trusted user query and generates
    Python code to fulfill the request. It never sees tool outputs
    or Q-LLM responses.
    """

    @abstractmethod
    def generate_code(
        self,
        user_query: str,
        tools: List[ToolSignature],
        system_context: Optional[str] = None,
        previous_code: Optional[str] = None,
        error_message: Optional[str] = None,
    ) -> PLLMResponse:
        """
        Generate Python code to fulfill the user query.

        Args:
            user_query: The natural language query from the user
            tools: List of available tool signatures
            system_context: Optional additional context (e.g., user name, date)
            previous_code: Previously generated code (for error recovery)
            error_message: Error from previous execution attempt

        Returns:
            PLLMResponse containing the generated code
        """
        pass


class QuarantinedLLM(ABC):
    """
    Abstract base class for the Quarantined LLM.

    The Q-LLM processes untrusted data without tool access.
    It parses unstructured data into structured output.
    """

    @abstractmethod
    def parse_data(
        self,
        query: str,
        output_schema: Type[T],
        context: Optional[str] = None,
    ) -> T:
        """
        Parse unstructured data into a structured format.

        Args:
            query: The query string containing data to parse
            output_schema: A Pydantic BaseModel class defining output structure
            context: Optional additional context

        Returns:
            An instance of output_schema populated with parsed data

        Raises:
            NotEnoughInformationError: If have_enough_information is False
        """
        pass


# ============================================================================
# System Prompts
# ============================================================================

PLLM_SYSTEM_PROMPT = """
You are a code generation assistant. Generate Python code to accomplish the user's task.

Follow these rules:
- Don't make assumptions about values to plug into functions
- Use the provided functions to disambiguate
- Do not assume the current year; use provided tools
- If provided with an email address, no need to search contacts
- Try to solve tasks without user intervention

BUILT-IN TYPES: NoneType, bool, int, float, str, list, tuple, dict, set

BUILT-IN FUNCTIONS: abs, any, all, bool, dir, divmod, enumerate, float, hash,
int, len, list, max, min, print, range, repr, reversed, set, sorted, str,
tuple, type, zip, sum

RESTRICTIONS:
- No while loops or generators
- No imports (use provided types only)
- No eval or exec
- No break or continue
- No function definitions (def/lambda)
- No side-effect methods (use comprehensions instead)

Use `query_ai_assistant(query: str, output_schema: type[T]) -> T` to parse
unstructured data into structured data. Always define a Pydantic BaseModel
for the output_schema.

IMPORTANT: You must define Pydantic models for structured data extraction.
Example:
```python
class EmailInfo(BaseModel):
    email_address: str
    name: str

info = query_ai_assistant(f"Extract email and name from: {data}", EmailInfo)
```

{tool_signatures}

{additional_context}

Respond with Python code in a markdown code block. The code should be complete
and directly executable by the interpreter.
"""

QLLM_SYSTEM_PROMPT = """
You are a data extraction assistant. Your task is to parse unstructured text
and extract structured information according to the provided schema.

IMPORTANT RULES:
1. You have NO access to any tools or external functions
2. You can ONLY process the data provided in the query
3. If you cannot find the required information, set have_enough_information to False
4. Do not make up or hallucinate information
5. Return valid JSON matching the schema exactly

If the requested information is not present in the provided text, you MUST
set have_enough_information to False in your response.
"""


# ============================================================================
# Concrete Implementations
# ============================================================================


class AnthropicPLLM(PrivilegedLLM):
    """
    Privileged LLM implementation using Anthropic's Claude API.
    """

    def __init__(
        self,
        api_key: Optional[str] = None,
        model: str = "claude-sonnet-4-20250514",
        temperature: float = 0.0,
        max_tokens: int = 4096,
    ):
        """
        Initialize the Anthropic P-LLM.

        Args:
            api_key: Anthropic API key (or use ANTHROPIC_API_KEY env var)
            model: Model to use
            temperature: Sampling temperature
            max_tokens: Maximum tokens in response
        """
        self._model = model
        self._temperature = temperature
        self._max_tokens = max_tokens
        self._client = None
        self._api_key = api_key

    def _get_client(self):
        """Lazy initialization of the Anthropic client."""
        if self._client is None:
            try:
                import anthropic
                self._client = anthropic.Anthropic(api_key=self._api_key)
            except ImportError:
                raise ImportError("anthropic package required. Install with: pip install anthropic")
        return self._client

    def _format_tool_signatures(self, tools: List[ToolSignature]) -> str:
        """Format tool signatures for the system prompt."""
        lines = ["AVAILABLE TOOLS:", ""]

        for tool in tools:
            lines.append(f"def {tool.name}(")
            params = tool.parameters.get("properties", {})
            required = tool.parameters.get("required", [])

            param_strs = []
            for param_name, param_schema in params.items():
                param_type = param_schema.get("type", "Any")
                if param_name in required:
                    param_strs.append(f"    {param_name}: {param_type}")
                else:
                    param_strs.append(f"    {param_name}: Optional[{param_type}] = None")

            lines.append(",\n".join(param_strs))
            lines.append(f") -> {tool.return_type}:")
            lines.append(f'    """{tool.description}"""')
            lines.append("")

        return "\n".join(lines)

    def generate_code(
        self,
        user_query: str,
        tools: List[ToolSignature],
        system_context: Optional[str] = None,
        previous_code: Optional[str] = None,
        error_message: Optional[str] = None,
    ) -> PLLMResponse:
        """Generate Python code using Claude."""
        client = self._get_client()

        # Build system prompt
        system_prompt = PLLM_SYSTEM_PROMPT.format(
            tool_signatures=self._format_tool_signatures(tools),
            additional_context=system_context or "",
        )

        # Build messages
        messages = []

        if previous_code:
            messages.append({
                "role": "assistant",
                "content": f"```python\n{previous_code}\n```",
            })

        if error_message:
            messages.append({
                "role": "user",
                "content": f"The previous code produced an error:\n{error_message}\n\nPlease fix the code.",
            })
        else:
            messages.append({
                "role": "user",
                "content": user_query,
            })

        # Call API
        response = client.messages.create(
            model=self._model,
            max_tokens=self._max_tokens,
            temperature=self._temperature,
            system=system_prompt,
            messages=messages,
        )

        # Extract code from response
        content = response.content[0].text
        code = self._extract_code(content)

        return PLLMResponse(code=code, explanation=content)

    def _extract_code(self, content: str) -> str:
        """Extract Python code from markdown code blocks."""
        import re

        # Try to find ```python ... ``` block
        pattern = r"```python\n(.*?)```"
        match = re.search(pattern, content, re.DOTALL)

        if match:
            return match.group(1).strip()

        # Try to find ``` ... ``` block
        pattern = r"```\n?(.*?)```"
        match = re.search(pattern, content, re.DOTALL)

        if match:
            return match.group(1).strip()

        # Return content as-is if no code block found
        return content.strip()


class AnthropicQLLM(QuarantinedLLM):
    """
    Quarantined LLM implementation using Anthropic's Claude API.
    """

    def __init__(
        self,
        api_key: Optional[str] = None,
        model: str = "claude-3-5-haiku-20241022",
        temperature: float = 0.0,
        max_tokens: int = 1024,
    ):
        """
        Initialize the Anthropic Q-LLM.

        Args:
            api_key: Anthropic API key
            model: Model to use (typically a smaller/cheaper model)
            temperature: Sampling temperature
            max_tokens: Maximum tokens in response
        """
        self._model = model
        self._temperature = temperature
        self._max_tokens = max_tokens
        self._client = None
        self._api_key = api_key

    def _get_client(self):
        """Lazy initialization of the Anthropic client."""
        if self._client is None:
            try:
                import anthropic
                self._client = anthropic.Anthropic(api_key=self._api_key)
            except ImportError:
                raise ImportError("anthropic package required. Install with: pip install anthropic")
        return self._client

    def parse_data(
        self,
        query: str,
        output_schema: Type[T],
        context: Optional[str] = None,
    ) -> T:
        """Parse unstructured data using Claude."""
        client = self._get_client()

        # Build the schema description
        schema_json = output_schema.model_json_schema()

        # Build prompt
        prompt = f"""Parse the following data and extract information according to the schema.

DATA TO PARSE:
{query}

{f"ADDITIONAL CONTEXT: {context}" if context else ""}

OUTPUT SCHEMA:
{schema_json}

IMPORTANT:
- If you cannot find the required information, set have_enough_information to False
- Return ONLY valid JSON matching the schema
- Do not include any explanation, just the JSON
"""

        # Call API
        response = client.messages.create(
            model=self._model,
            max_tokens=self._max_tokens,
            temperature=self._temperature,
            system=QLLM_SYSTEM_PROMPT,
            messages=[{"role": "user", "content": prompt}],
        )

        # Parse response
        content = response.content[0].text

        # Extract JSON from response
        import json
        import re

        # Try to find JSON in code blocks
        json_match = re.search(r"```(?:json)?\n?(.*?)```", content, re.DOTALL)
        if json_match:
            json_str = json_match.group(1)
        else:
            json_str = content

        try:
            data = json.loads(json_str.strip())
        except json.JSONDecodeError:
            # Try to find JSON object in the content
            match = re.search(r"\{.*\}", content, re.DOTALL)
            if match:
                data = json.loads(match.group())
            else:
                raise NotEnoughInformationError("Could not parse Q-LLM response as JSON")

        # Create the output object
        result = output_schema.model_validate(data)

        # Check have_enough_information
        if hasattr(result, "have_enough_information") and not result.have_enough_information:
            raise NotEnoughInformationError("Q-LLM indicated insufficient information")

        return result


# ============================================================================
# Mock LLM implementations for testing
# ============================================================================


class MockPLLM(PrivilegedLLM):
    """Mock P-LLM for testing."""

    def __init__(self, responses: Optional[List[str]] = None):
        self._responses = responses or []
        self._call_count = 0

    def generate_code(
        self,
        user_query: str,
        tools: List[ToolSignature],
        system_context: Optional[str] = None,
        previous_code: Optional[str] = None,
        error_message: Optional[str] = None,
    ) -> PLLMResponse:
        if self._call_count < len(self._responses):
            code = self._responses[self._call_count]
            self._call_count += 1
            return PLLMResponse(code=code)

        # Default: return a simple print statement
        return PLLMResponse(code='print("No more mock responses")')


class MockQLLM(QuarantinedLLM):
    """Mock Q-LLM for testing."""

    def __init__(self, responses: Optional[Dict[Type, Any]] = None):
        self._responses = responses or {}

    def add_response(self, schema: Type[T], value: T) -> None:
        """Add a mock response for a schema type."""
        self._responses[schema] = value

    def parse_data(
        self,
        query: str,
        output_schema: Type[T],
        context: Optional[str] = None,
    ) -> T:
        if output_schema in self._responses:
            return self._responses[output_schema]

        raise NotEnoughInformationError(f"No mock response for {output_schema}")
