"""
Tool system for the CaMeL interpreter.

This module provides:
- ToolDefinition and ToolSignature data classes
- ToolRegistry for managing available tools
- Decorator for registering tools
- Example tool implementations
"""

import inspect
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional, Type, Union, get_type_hints

from .types import Capability, CaMeLValue


@dataclass
class ToolSignature:
    """
    Description of a tool available to the P-LLM.

    This is the information P-LLM uses to generate code using the tool.
    """
    name: str
    description: str
    parameters: Dict[str, Any]  # JSON Schema format
    return_type: str


@dataclass
class ToolDefinition:
    """
    Complete definition of a tool.

    Contains the function implementation and metadata needed for
    execution and capability annotation.
    """
    name: str
    function: Callable
    description: str
    parameters: Dict[str, Any]
    return_type: Type
    capability_annotator: Optional[Callable[[Any, Dict[str, CaMeLValue]], Capability]] = None


def _python_type_to_json_schema(python_type: Type) -> Dict[str, Any]:
    """
    Convert a Python type annotation to JSON Schema.

    Args:
        python_type: Python type annotation

    Returns:
        JSON Schema dictionary
    """
    # Handle basic types
    type_map = {
        str: {"type": "string"},
        int: {"type": "integer"},
        float: {"type": "number"},
        bool: {"type": "boolean"},
        type(None): {"type": "null"},
    }

    if python_type in type_map:
        return type_map[python_type]

    # Handle Optional
    origin = getattr(python_type, "__origin__", None)
    args = getattr(python_type, "__args__", ())

    if origin is Union and type(None) in args:
        # Optional type
        non_none_args = [a for a in args if a is not type(None)]
        if len(non_none_args) == 1:
            inner_schema = _python_type_to_json_schema(non_none_args[0])
            return {**inner_schema, "nullable": True}

    # Handle List
    if origin is list:
        if args:
            return {
                "type": "array",
                "items": _python_type_to_json_schema(args[0]),
            }
        return {"type": "array"}

    # Handle Dict
    if origin is dict:
        return {"type": "object"}

    # Default to string for unknown types
    return {"type": "string", "description": f"Type: {python_type}"}


class ToolRegistry:
    """
    Registry for managing available tools.

    Provides methods to register tools via decorator or directly,
    and to retrieve tool definitions and signatures.
    """

    def __init__(self):
        self._tools: Dict[str, ToolDefinition] = {}

    def register(
        self,
        name: str,
        description: str,
        capability_annotator: Optional[Callable] = None,
    ) -> Callable[[Callable], Callable]:
        """
        Decorator to register a tool.

        Args:
            name: Name of the tool
            description: Human-readable description for P-LLM
            capability_annotator: Optional function to annotate output capabilities

        Returns:
            Decorator function

        Example:
            @tool_registry.register(
                name="send_email",
                description="Sends an email to the specified recipients"
            )
            def send_email(recipients: List[str], subject: str, body: str) -> Email:
                ...
        """
        def decorator(func: Callable) -> Callable:
            # Extract parameter schema from type hints
            sig = inspect.signature(func)
            hints = get_type_hints(func) if hasattr(func, "__annotations__") else {}

            parameters: Dict[str, Any] = {}
            required: List[str] = []

            for param_name, param in sig.parameters.items():
                if param_name in hints:
                    param_type = hints[param_name]
                    parameters[param_name] = _python_type_to_json_schema(param_type)

                    # Check if parameter is required (no default value)
                    if param.default is inspect.Parameter.empty:
                        required.append(param_name)

            return_type = hints.get("return", Any)

            self._tools[name] = ToolDefinition(
                name=name,
                function=func,
                description=description,
                parameters={
                    "type": "object",
                    "properties": parameters,
                    "required": required,
                },
                return_type=return_type,
                capability_annotator=capability_annotator,
            )

            return func

        return decorator

    def register_tool(
        self,
        name: str,
        function: Callable,
        description: str,
        capability_annotator: Optional[Callable] = None,
    ) -> None:
        """
        Register a tool directly (non-decorator form).

        Args:
            name: Name of the tool
            function: The tool function
            description: Human-readable description
            capability_annotator: Optional capability annotator
        """
        # Use the decorator internally
        decorator = self.register(name, description, capability_annotator)
        decorator(function)

    def get(self, name: str) -> Optional[ToolDefinition]:
        """
        Get a tool by name.

        Args:
            name: Tool name

        Returns:
            ToolDefinition or None if not found
        """
        return self._tools.get(name)

    def get_all(self) -> Dict[str, ToolDefinition]:
        """
        Get all registered tools.

        Returns:
            Dictionary mapping tool names to definitions
        """
        return dict(self._tools)

    def get_signatures(self) -> List[ToolSignature]:
        """
        Get tool signatures for P-LLM.

        Returns:
            List of ToolSignature objects
        """
        return [
            ToolSignature(
                name=t.name,
                description=t.description,
                parameters=t.parameters,
                return_type=t.return_type.__name__ if hasattr(t.return_type, "__name__") else str(t.return_type),
            )
            for t in self._tools.values()
        ]

    def get_signature_text(self) -> str:
        """
        Get formatted tool signatures for P-LLM system prompt.

        Returns:
            Formatted string describing all available tools
        """
        lines = ["Available Tools:", ""]

        for tool in self._tools.values():
            lines.append(f"## {tool.name}")
            lines.append(f"Description: {tool.description}")
            lines.append("Parameters:")

            params = tool.parameters.get("properties", {})
            required = tool.parameters.get("required", [])

            for param_name, param_schema in params.items():
                req_marker = "(required)" if param_name in required else "(optional)"
                param_type = param_schema.get("type", "any")
                lines.append(f"  - {param_name}: {param_type} {req_marker}")

            lines.append(f"Returns: {tool.return_type.__name__ if hasattr(tool.return_type, '__name__') else str(tool.return_type)}")
            lines.append("")

        return "\n".join(lines)

    def __contains__(self, name: str) -> bool:
        return name in self._tools

    def __len__(self) -> int:
        return len(self._tools)


# Global registry instance
tool_registry = ToolRegistry()


# ============================================================================
# Example Tool Capability Annotators
# ============================================================================


def email_capability_annotator(result: Any, args: Dict[str, CaMeLValue]) -> Capability:
    """
    Annotate email tool outputs with sender/recipient readers.

    Args:
        result: The email object returned by the tool
        args: The arguments passed to the tool

    Returns:
        Capability with appropriate readers
    """
    from .types import DataSource, Public

    readers = set()

    if hasattr(result, "sender"):
        readers.add(result.sender)
    if hasattr(result, "recipients"):
        readers.update(result.recipients)
    if hasattr(result, "cc"):
        readers.update(result.cc)
    if hasattr(result, "bcc"):
        readers.update(result.bcc)

    return Capability(
        sources={DataSource(source_type="Tool", tool_id="email", inner_source="email_participants")},
        readers=readers if readers else Public,
    )


def file_capability_annotator(result: Any, args: Dict[str, CaMeLValue]) -> Capability:
    """
    Annotate file tool outputs with sharing info as readers.

    Args:
        result: The file object returned by the tool
        args: The arguments passed to the tool

    Returns:
        Capability with file sharing as readers
    """
    from .types import DataSource, Public

    readers = set()

    if hasattr(result, "owner"):
        readers.add(result.owner)
    if hasattr(result, "shared_with"):
        readers.update(result.shared_with.keys())

    return Capability(
        sources={DataSource(source_type="Tool", tool_id="file", inner_source="file_editors")},
        readers=readers if readers else Public,
    )
