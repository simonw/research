"""Tests for CaMeL security policies."""

import pytest

from camel.policies import (
    SecurityPolicy,
    SecurityPolicyResult,
    PolicyDecision,
    PolicyRegistry,
    PolicyBuilder,
    SendEmailPolicy,
    CreateCalendarEventPolicy,
    SendMoneyPolicy,
    is_trusted,
    can_readers_read_value,
)
from camel.types import (
    CaMeLValue,
    Capability,
    CapabilityAssigner,
    DataSource,
    Public,
)


class TestPolicyHelpers:
    """Tests for policy helper functions."""

    def test_is_trusted_user_source(self):
        """Test that User source is trusted."""
        value = CapabilityAssigner.from_user_literal("test")
        assert is_trusted(value) is True

    def test_is_trusted_tool_source(self):
        """Test that Tool source is not trusted."""
        value = CapabilityAssigner.from_tool_output(
            "test",
            tool_name="some_tool",
        )
        assert is_trusted(value) is False

    def test_is_trusted_with_dependencies(self):
        """Test trust checking with dependencies."""
        # User source
        user_value = CapabilityAssigner.from_user_literal("trusted")

        # Tool source
        tool_value = CapabilityAssigner.from_tool_output(
            "untrusted",
            tool_name="get_data",
        )

        # Value that depends on both
        mixed_value = CaMeLValue(
            raw="mixed",
            capability=Capability(sources={DataSource(source_type="CaMeL")}),
            dependencies={user_value, tool_value},
        )

        assert is_trusted(mixed_value) is False

    def test_can_readers_read_public(self):
        """Test that anyone can read Public values."""
        value = CaMeLValue(
            raw="test",
            capability=Capability(readers=Public),
        )

        assert can_readers_read_value({"anyone@example.com"}, value) is True

    def test_can_readers_read_restricted(self):
        """Test reader restriction checking."""
        value = CaMeLValue(
            raw="test",
            capability=Capability(
                readers={"alice@example.com", "bob@example.com"},
            ),
        )

        # Alice can read
        assert can_readers_read_value({"alice@example.com"}, value) is True

        # Both can read
        assert can_readers_read_value(
            {"alice@example.com", "bob@example.com"},
            value,
        ) is True

        # Charlie cannot read
        assert can_readers_read_value({"charlie@example.com"}, value) is False

        # Alice and Charlie together - fails because Charlie can't read
        assert can_readers_read_value(
            {"alice@example.com", "charlie@example.com"},
            value,
        ) is False


class TestSendEmailPolicy:
    """Tests for SendEmailPolicy."""

    def test_allows_trusted_recipients(self):
        """Test that trusted recipients are allowed."""
        policy = SendEmailPolicy()

        recipients = CapabilityAssigner.from_user_literal(["alice@example.com"])
        body = CapabilityAssigner.from_tool_output(
            "secret data",
            "get_file",
            readers={"bob@example.com"},  # Bob can read, not Alice
        )
        subject = CapabilityAssigner.from_user_literal("Subject")

        decision = policy.check(
            "send_email",
            {"recipients": recipients, "body": body, "subject": subject},
            None,
        )

        # Should be allowed because recipients came from user (trusted)
        assert decision.result == SecurityPolicyResult.ALLOWED

    def test_blocks_unauthorized_readers(self):
        """Test that unauthorized readers are blocked."""
        policy = SendEmailPolicy()

        # Recipients from untrusted source
        recipients = CapabilityAssigner.from_tool_output(
            ["attacker@evil.com"],
            "get_contacts",
        )
        body = CapabilityAssigner.from_tool_output(
            "confidential data",
            "get_file",
            readers={"alice@example.com"},  # Only Alice can read
        )
        subject = CapabilityAssigner.from_user_literal("Subject")

        decision = policy.check(
            "send_email",
            {"recipients": recipients, "body": body, "subject": subject},
            None,
        )

        # Should be denied because attacker@evil.com cannot read the body
        assert decision.result == SecurityPolicyResult.DENIED
        assert "body" in decision.reason.lower()

    def test_allows_authorized_readers(self):
        """Test that authorized readers are allowed."""
        policy = SendEmailPolicy()

        recipients = CapabilityAssigner.from_tool_output(
            ["alice@example.com"],
            "get_contacts",
        )
        body = CapabilityAssigner.from_tool_output(
            "shared data",
            "get_file",
            readers={"alice@example.com", "bob@example.com"},
        )
        subject = CapabilityAssigner.from_user_literal("Subject")

        decision = policy.check(
            "send_email",
            {"recipients": recipients, "body": body, "subject": subject},
            None,
        )

        # Should be allowed because alice@example.com can read the body
        assert decision.result == SecurityPolicyResult.ALLOWED

    def test_ignores_other_tools(self):
        """Test that policy ignores non-send_email tools."""
        policy = SendEmailPolicy()

        decision = policy.check("other_tool", {}, None)

        assert decision.result == SecurityPolicyResult.ALLOWED


class TestCreateCalendarEventPolicy:
    """Tests for CreateCalendarEventPolicy."""

    def test_allows_trusted_participants(self):
        """Test that trusted participants are allowed."""
        policy = CreateCalendarEventPolicy()

        participants = CapabilityAssigner.from_user_literal(
            ["alice@example.com", "bob@example.com"]
        )
        title = CapabilityAssigner.from_tool_output(
            "Secret Meeting",
            "get_data",
            readers={"charlie@example.com"},  # Charlie can read, not participants
        )

        decision = policy.check(
            "create_calendar_event",
            {"participants": participants, "title": title},
            None,
        )

        # Should be allowed because participants came from user (trusted)
        assert decision.result == SecurityPolicyResult.ALLOWED

    def test_blocks_unauthorized_participants(self):
        """Test that unauthorized participants are blocked."""
        policy = CreateCalendarEventPolicy()

        participants = CapabilityAssigner.from_tool_output(
            ["outsider@example.com"],
            "get_contacts",
        )
        title = CapabilityAssigner.from_tool_output(
            "Confidential Meeting",
            "get_data",
            readers={"alice@example.com"},
        )

        decision = policy.check(
            "create_calendar_event",
            {"participants": participants, "title": title},
            None,
        )

        assert decision.result == SecurityPolicyResult.DENIED


class TestSendMoneyPolicy:
    """Tests for SendMoneyPolicy."""

    def test_requires_trusted_recipient(self):
        """Test that recipient must be trusted."""
        policy = SendMoneyPolicy()

        recipient = CapabilityAssigner.from_tool_output(
            "attacker@evil.com",
            "get_contacts",
        )
        amount = CapabilityAssigner.from_user_literal(100.0)

        decision = policy.check(
            "send_money",
            {"recipient": recipient, "amount": amount},
            None,
        )

        assert decision.result == SecurityPolicyResult.DENIED
        assert "recipient" in decision.reason.lower()

    def test_requires_trusted_amount(self):
        """Test that amount must be trusted."""
        policy = SendMoneyPolicy()

        recipient = CapabilityAssigner.from_user_literal("alice@example.com")
        amount = CapabilityAssigner.from_tool_output(
            1000000.0,  # Manipulated amount
            "get_data",
        )

        decision = policy.check(
            "send_money",
            {"recipient": recipient, "amount": amount},
            None,
        )

        assert decision.result == SecurityPolicyResult.DENIED
        assert "amount" in decision.reason.lower()

    def test_allows_trusted_transfer(self):
        """Test that fully trusted transfers are allowed."""
        policy = SendMoneyPolicy()

        recipient = CapabilityAssigner.from_user_literal("alice@example.com")
        amount = CapabilityAssigner.from_user_literal(50.0)

        decision = policy.check(
            "send_money",
            {"recipient": recipient, "amount": amount},
            None,
        )

        assert decision.result == SecurityPolicyResult.ALLOWED


class TestPolicyRegistry:
    """Tests for PolicyRegistry."""

    def test_register_tool_policy(self):
        """Test registering a tool-specific policy."""
        registry = PolicyRegistry()

        class TestPolicy(SecurityPolicy):
            def check(self, tool_name, kwargs, memory_state):
                return PolicyDecision(SecurityPolicyResult.ALLOWED)

        policy = TestPolicy()
        registry.register_tool_policy("my_tool", policy)

        policies = registry.get_policies_for_tool("my_tool")
        assert policy in policies

    def test_register_global_policy(self):
        """Test registering a global policy."""
        registry = PolicyRegistry()

        class GlobalPolicy(SecurityPolicy):
            def check(self, tool_name, kwargs, memory_state):
                return PolicyDecision(SecurityPolicyResult.ALLOWED)

        policy = GlobalPolicy()
        registry.register_global_policy(policy)

        # Global policy applies to all tools
        assert policy in registry.get_policies_for_tool("any_tool")
        assert policy in registry.get_policies_for_tool("another_tool")

    def test_check_all_stops_on_deny(self):
        """Test that check_all stops on first denial."""
        registry = PolicyRegistry()

        class DenyPolicy(SecurityPolicy):
            def check(self, tool_name, kwargs, memory_state):
                return PolicyDecision(
                    SecurityPolicyResult.DENIED,
                    reason="Denied by test",
                )

        class AllowPolicy(SecurityPolicy):
            def check(self, tool_name, kwargs, memory_state):
                return PolicyDecision(SecurityPolicyResult.ALLOWED)

        registry.register_global_policy(DenyPolicy())
        registry.register_global_policy(AllowPolicy())

        decision = registry.check_all("any_tool", {}, None)
        assert decision.result == SecurityPolicyResult.DENIED


class TestPolicyBuilder:
    """Tests for PolicyBuilder DSL."""

    def test_require_trusted_source(self):
        """Test require_trusted_source check."""
        policy = (
            PolicyBuilder("test_tool")
            .require_trusted_source("param1")
            .build()
        )

        # Trusted source should pass
        trusted = CapabilityAssigner.from_user_literal("value")
        decision = policy.check("test_tool", {"param1": trusted}, None)
        assert decision.result == SecurityPolicyResult.ALLOWED

        # Untrusted source should fail
        untrusted = CapabilityAssigner.from_tool_output("value", "other_tool")
        decision = policy.check("test_tool", {"param1": untrusted}, None)
        assert decision.result == SecurityPolicyResult.DENIED

    def test_require_readers_can_read(self):
        """Test require_readers_can_read check."""
        policy = (
            PolicyBuilder("test_tool")
            .require_readers_can_read("recipients", "data")
            .build()
        )

        recipients = CapabilityAssigner.from_tool_output(
            ["alice@example.com"],
            "get_contacts",
        )

        # Data readable by recipient
        readable_data = CapabilityAssigner.from_tool_output(
            "content",
            "get_file",
            readers={"alice@example.com", "bob@example.com"},
        )
        decision = policy.check(
            "test_tool",
            {"recipients": recipients, "data": readable_data},
            None,
        )
        assert decision.result == SecurityPolicyResult.ALLOWED

        # Data not readable by recipient
        unreadable_data = CapabilityAssigner.from_tool_output(
            "content",
            "get_file",
            readers={"bob@example.com"},
        )
        decision = policy.check(
            "test_tool",
            {"recipients": recipients, "data": unreadable_data},
            None,
        )
        assert decision.result == SecurityPolicyResult.DENIED

    def test_custom_check(self):
        """Test custom check function."""
        def no_empty_strings(kwargs):
            for key, value in kwargs.items():
                if isinstance(value.raw, str) and value.raw == "":
                    return f"{key} cannot be empty"
            return None

        policy = (
            PolicyBuilder("test_tool")
            .custom_check(no_empty_strings)
            .build()
        )

        # Non-empty should pass
        decision = policy.check(
            "test_tool",
            {"data": CapabilityAssigner.from_user_literal("content")},
            None,
        )
        assert decision.result == SecurityPolicyResult.ALLOWED

        # Empty should fail
        decision = policy.check(
            "test_tool",
            {"data": CapabilityAssigner.from_user_literal("")},
            None,
        )
        assert decision.result == SecurityPolicyResult.DENIED
