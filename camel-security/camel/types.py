"""
Core data types for the CaMeL system.

This module defines the fundamental types used for capability tracking:
- DataSource: Represents the origin of data
- Capability: Tags attached to values tracking sources and readers
- CaMeLValue: Wrapper for all values with capability metadata
"""

from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Generic, Optional, Set, TypeVar, Union

T = TypeVar("T")


class PublicSingleton:
    """
    Singleton representing public/unrestricted readers.

    When a value has Public as its readers, any recipient can access it.
    """
    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __repr__(self):
        return "Public"

    def __eq__(self, other):
        return isinstance(other, PublicSingleton)

    def __hash__(self):
        return hash("PublicSingleton")


# Global Public singleton instance
Public = PublicSingleton()


@dataclass(frozen=True)
class DataSource:
    """
    Represents the origin of data.

    Attributes:
        source_type: The type of source - "User", "CaMeL", or tool name
        tool_id: Identifier for the specific tool (if source_type is "Tool")
        inner_source: Additional source classification (e.g., "file_editors")
        timestamp: When this source was recorded
    """
    source_type: str
    tool_id: Optional[str] = None
    inner_source: Optional[str] = None
    timestamp: datetime = field(default_factory=datetime.now, compare=False)

    def __repr__(self):
        parts = [f"Source({self.source_type}"]
        if self.tool_id:
            parts.append(f":{self.tool_id}")
        if self.inner_source:
            parts.append(f"[{self.inner_source}]")
        parts.append(")")
        return "".join(parts)


@dataclass
class Capability:
    """
    Capability tags attached to values.

    Capabilities track:
    - sources: Where the data came from (union when merging)
    - readers: Who is allowed to read this data (intersection when merging)

    The security model uses these to enforce policies about data flow.
    """
    sources: Set[DataSource] = field(default_factory=set)
    readers: Union[Set[str], PublicSingleton] = field(default_factory=lambda: Public)

    def merge_with(self, other: "Capability") -> "Capability":
        """
        Merge two capabilities.

        Sources are unioned (data comes from all original sources).
        Readers are intersected (only those who can read both can read result).

        Args:
            other: Another capability to merge with

        Returns:
            New Capability with merged sources and readers
        """
        new_sources = self.sources | other.sources

        if isinstance(self.readers, PublicSingleton):
            new_readers = other.readers
        elif isinstance(other.readers, PublicSingleton):
            new_readers = self.readers
        else:
            new_readers = self.readers & other.readers

        return Capability(sources=new_sources, readers=new_readers)

    def __repr__(self):
        sources_str = ", ".join(str(s) for s in self.sources)
        return f"Capability(sources={{{sources_str}}}, readers={self.readers})"


class CaMeLValue(Generic[T]):
    """
    Wrapper for all values in the CaMeL interpreter.

    Tracks the raw value along with its capability metadata and
    dependencies on other values.

    Type Parameters:
        T: The type of the underlying raw value

    Attributes:
        _raw: The underlying Python value
        _capability: Capability metadata for this value
        _dependencies: Set of CaMeLValues this value depends on
    """

    def __init__(
        self,
        raw: T,
        capability: Optional[Capability] = None,
        dependencies: Optional[Set["CaMeLValue"]] = None,
    ):
        """
        Initialize a CaMeLValue.

        Args:
            raw: The underlying Python value
            capability: Capability metadata (defaults to empty Capability)
            dependencies: Set of values this depends on (defaults to empty)
        """
        self._raw = raw
        self._capability = capability if capability is not None else Capability()
        self._dependencies = dependencies if dependencies is not None else set()

    @property
    def raw(self) -> T:
        """Get the underlying raw value."""
        return self._raw

    @property
    def capability(self) -> Capability:
        """Get the capability metadata."""
        return self._capability

    @property
    def dependencies(self) -> Set["CaMeLValue"]:
        """Get direct dependencies of this value."""
        return self._dependencies

    def get_full_dependency_graph(self) -> Set["CaMeLValue"]:
        """
        Recursively get all dependencies (transitive closure).

        Returns:
            Set of all CaMeLValues this value transitively depends on
        """
        all_deps: Set[CaMeLValue] = set()
        to_visit = list(self._dependencies)

        while to_visit:
            dep = to_visit.pop()
            if dep not in all_deps:
                all_deps.add(dep)
                to_visit.extend(dep._dependencies)

        return all_deps

    def get_merged_capability(self) -> Capability:
        """
        Get capability merged with all dependencies.

        This computes the effective capability by merging this value's
        capability with all capabilities from the dependency graph.

        Returns:
            Merged Capability representing the full provenance
        """
        result = self._capability
        for dep in self.get_full_dependency_graph():
            result = result.merge_with(dep.capability)
        return result

    def __repr__(self):
        return f"CaMeLValue({self._raw!r}, cap={self._capability})"

    def __eq__(self, other):
        if isinstance(other, CaMeLValue):
            return self._raw == other._raw
        return self._raw == other

    def __hash__(self):
        # Use id for hashing since raw values may not be hashable
        return id(self)


# Type aliases for common wrapped types
CaMeLStr = CaMeLValue[str]
CaMeLInt = CaMeLValue[int]
CaMeLFloat = CaMeLValue[float]
CaMeLBool = CaMeLValue[bool]
CaMeLList = CaMeLValue[list]
CaMeLDict = CaMeLValue[dict]


class CapabilityAssigner:
    """
    Factory methods for creating CaMeLValues with appropriate capabilities.

    This class provides static methods to create properly annotated values
    based on their origin (user input, tool output, Q-LLM output, etc.).
    """

    @staticmethod
    def from_user_literal(value: Any) -> CaMeLValue:
        """
        Create a CaMeLValue from a user-provided literal in P-LLM code.

        Literals in P-LLM generated code are considered trusted since
        P-LLM only sees the user query.

        Args:
            value: The literal value

        Returns:
            CaMeLValue with User source and Public readers
        """
        return CaMeLValue(
            raw=value,
            capability=Capability(
                sources={DataSource(source_type="User")},
                readers=Public,
            ),
        )

    @staticmethod
    def from_tool_output(
        value: Any,
        tool_name: str,
        readers: Union[Set[str], PublicSingleton] = None,
        inner_source: Optional[str] = None,
    ) -> CaMeLValue:
        """
        Create a CaMeLValue from a tool output.

        Args:
            value: The tool's return value
            tool_name: Name of the tool that produced this value
            readers: Set of principals who can read this value
            inner_source: Additional source classification

        Returns:
            CaMeLValue tagged with tool source and specified readers
        """
        return CaMeLValue(
            raw=value,
            capability=Capability(
                sources={
                    DataSource(
                        source_type="Tool",
                        tool_id=tool_name,
                        inner_source=inner_source,
                    )
                },
                readers=readers if readers is not None else Public,
            ),
        )

    @staticmethod
    def from_qllm_output(
        value: Any,
        input_dependencies: Set[CaMeLValue],
    ) -> CaMeLValue:
        """
        Create a CaMeLValue from Q-LLM output, inheriting dependencies.

        The Q-LLM parses unstructured data into structured output. The
        resulting value inherits all dependencies from its inputs since
        the Q-LLM could have been influenced by any of them.

        Args:
            value: The Q-LLM's structured output
            input_dependencies: Values that were passed to the Q-LLM

        Returns:
            CaMeLValue with Q-LLM source and inherited dependencies
        """
        return CaMeLValue(
            raw=value,
            capability=Capability(
                sources={DataSource(source_type="CaMeL", tool_id="query_ai_assistant")}
            ),
            dependencies=input_dependencies,
        )

    @staticmethod
    def from_operation(
        value: Any,
        operand_dependencies: Set[CaMeLValue],
    ) -> CaMeLValue:
        """
        Create a CaMeLValue from an operation on other values.

        When performing operations (e.g., string concatenation, arithmetic),
        the result depends on all operands.

        Args:
            value: The computed value
            operand_dependencies: CaMeLValues used in the computation

        Returns:
            CaMeLValue with CaMeL source and specified dependencies
        """
        return CaMeLValue(
            raw=value,
            capability=Capability(sources={DataSource(source_type="CaMeL")}),
            dependencies=operand_dependencies,
        )
