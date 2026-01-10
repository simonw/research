"""Tests for CaMeL core types."""

import pytest
from datetime import datetime

from camel.types import (
    CaMeLValue,
    Capability,
    CapabilityAssigner,
    DataSource,
    Public,
    PublicSingleton,
)


class TestDataSource:
    """Tests for DataSource."""

    def test_user_source(self):
        """Test creating a user data source."""
        source = DataSource(source_type="User")
        assert source.source_type == "User"
        assert source.tool_id is None
        assert source.inner_source is None

    def test_tool_source(self):
        """Test creating a tool data source."""
        source = DataSource(
            source_type="Tool",
            tool_id="send_email",
            inner_source="email_participants",
        )
        assert source.source_type == "Tool"
        assert source.tool_id == "send_email"
        assert source.inner_source == "email_participants"

    def test_source_repr(self):
        """Test string representation."""
        source = DataSource(source_type="Tool", tool_id="get_file")
        assert "Tool" in repr(source)
        assert "get_file" in repr(source)


class TestPublicSingleton:
    """Tests for PublicSingleton."""

    def test_singleton_identity(self):
        """Test that Public is a singleton."""
        p1 = PublicSingleton()
        p2 = PublicSingleton()
        assert p1 is p2
        assert p1 is Public

    def test_repr(self):
        """Test string representation."""
        assert repr(Public) == "Public"

    def test_equality(self):
        """Test equality."""
        assert Public == PublicSingleton()


class TestCapability:
    """Tests for Capability."""

    def test_empty_capability(self):
        """Test default empty capability."""
        cap = Capability()
        assert len(cap.sources) == 0
        assert cap.readers is Public

    def test_capability_with_sources(self):
        """Test capability with sources."""
        source = DataSource(source_type="User")
        cap = Capability(sources={source})
        assert len(cap.sources) == 1
        assert source in cap.sources

    def test_capability_with_readers(self):
        """Test capability with specific readers."""
        cap = Capability(readers={"alice@example.com", "bob@example.com"})
        assert "alice@example.com" in cap.readers
        assert "bob@example.com" in cap.readers

    def test_merge_sources_union(self):
        """Test that merging capabilities unions sources."""
        source1 = DataSource(source_type="User")
        source2 = DataSource(source_type="Tool", tool_id="get_file")

        cap1 = Capability(sources={source1})
        cap2 = Capability(sources={source2})

        merged = cap1.merge_with(cap2)

        assert source1 in merged.sources
        assert source2 in merged.sources

    def test_merge_readers_intersection(self):
        """Test that merging capabilities intersects readers."""
        cap1 = Capability(readers={"alice@example.com", "bob@example.com"})
        cap2 = Capability(readers={"bob@example.com", "charlie@example.com"})

        merged = cap1.merge_with(cap2)

        assert merged.readers == {"bob@example.com"}

    def test_merge_with_public_readers(self):
        """Test merging when one has Public readers."""
        cap1 = Capability(readers=Public)
        cap2 = Capability(readers={"alice@example.com"})

        merged = cap1.merge_with(cap2)

        assert merged.readers == {"alice@example.com"}

        # Test reverse order
        merged2 = cap2.merge_with(cap1)
        assert merged2.readers == {"alice@example.com"}


class TestCaMeLValue:
    """Tests for CaMeLValue."""

    def test_basic_value(self):
        """Test creating a basic CaMeLValue."""
        value = CaMeLValue(raw="hello")
        assert value.raw == "hello"
        assert isinstance(value.capability, Capability)
        assert len(value.dependencies) == 0

    def test_value_with_capability(self):
        """Test CaMeLValue with explicit capability."""
        cap = Capability(sources={DataSource(source_type="User")})
        value = CaMeLValue(raw=42, capability=cap)

        assert value.raw == 42
        assert len(value.capability.sources) == 1

    def test_value_with_dependencies(self):
        """Test CaMeLValue with dependencies."""
        dep1 = CaMeLValue(raw="dep1")
        dep2 = CaMeLValue(raw="dep2")

        value = CaMeLValue(raw="result", dependencies={dep1, dep2})

        assert len(value.dependencies) == 2
        assert dep1 in value.dependencies
        assert dep2 in value.dependencies

    def test_get_full_dependency_graph(self):
        """Test transitive dependency resolution."""
        # Create a chain: v1 -> v2 -> v3
        v1 = CaMeLValue(raw="v1")
        v2 = CaMeLValue(raw="v2", dependencies={v1})
        v3 = CaMeLValue(raw="v3", dependencies={v2})

        deps = v3.get_full_dependency_graph()

        assert v1 in deps
        assert v2 in deps
        assert len(deps) == 2

    def test_get_merged_capability(self):
        """Test capability merging through dependencies."""
        source1 = DataSource(source_type="User")
        source2 = DataSource(source_type="Tool", tool_id="get_file")

        v1 = CaMeLValue(
            raw="v1",
            capability=Capability(
                sources={source1},
                readers={"alice@example.com", "bob@example.com"},
            ),
        )
        v2 = CaMeLValue(
            raw="v2",
            capability=Capability(
                sources={source2},
                readers={"bob@example.com", "charlie@example.com"},
            ),
            dependencies={v1},
        )

        merged = v2.get_merged_capability()

        # Sources should be unioned
        assert source1 in merged.sources
        assert source2 in merged.sources

        # Readers should be intersected
        assert merged.readers == {"bob@example.com"}


class TestCapabilityAssigner:
    """Tests for CapabilityAssigner factory methods."""

    def test_from_user_literal(self):
        """Test creating value from user literal."""
        value = CapabilityAssigner.from_user_literal("hello")

        assert value.raw == "hello"
        assert any(s.source_type == "User" for s in value.capability.sources)
        assert value.capability.readers is Public

    def test_from_tool_output(self):
        """Test creating value from tool output."""
        value = CapabilityAssigner.from_tool_output(
            value={"content": "file data"},
            tool_name="get_file",
            readers={"alice@example.com"},
            inner_source="file_editors",
        )

        assert value.raw == {"content": "file data"}
        assert any(
            s.source_type == "Tool" and s.tool_id == "get_file"
            for s in value.capability.sources
        )
        assert value.capability.readers == {"alice@example.com"}

    def test_from_qllm_output(self):
        """Test creating value from Q-LLM output."""
        dep = CaMeLValue(raw="input data")

        value = CapabilityAssigner.from_qllm_output(
            value={"extracted": "info"},
            input_dependencies={dep},
        )

        assert value.raw == {"extracted": "info"}
        assert dep in value.dependencies
        assert any(
            s.tool_id == "query_ai_assistant"
            for s in value.capability.sources
        )

    def test_from_operation(self):
        """Test creating value from operation."""
        v1 = CaMeLValue(raw=10)
        v2 = CaMeLValue(raw=20)

        result = CapabilityAssigner.from_operation(30, {v1, v2})

        assert result.raw == 30
        assert v1 in result.dependencies
        assert v2 in result.dependencies
