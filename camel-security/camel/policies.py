"""
Security policy framework for the CaMeL system.

This module provides:
- Base SecurityPolicy class and result types
- Policy registry for managing policies
- Helper functions for policy checks
- Built-in policy implementations
- PolicyBuilder DSL for creating policies
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from enum import Enum, auto
from typing import Any, Callable, Dict, List, Mapping, Optional, Set, Union

from .types import CaMeLValue, Capability, PublicSingleton, Public


class SecurityPolicyResult(Enum):
    """Result of a security policy check."""
    ALLOWED = auto()
    DENIED = auto()
    REQUIRES_CONFIRMATION = auto()


@dataclass
class PolicyDecision:
    """
    Detailed result of a security policy evaluation.

    Attributes:
        result: Whether the action is allowed, denied, or needs confirmation
        reason: Human-readable explanation of the decision
        details: Additional context about the decision
    """
    result: SecurityPolicyResult
    reason: Optional[str] = None
    details: Optional[dict] = None


class SecurityPolicy(ABC):
    """
    Abstract base class for security policies.

    Security policies check tool calls against data flow information
    to determine if they should be allowed.
    """

    @abstractmethod
    def check(
        self,
        tool_name: str,
        kwargs: Mapping[str, CaMeLValue],
        memory_state: Any,
    ) -> PolicyDecision:
        """
        Check if a tool call is allowed.

        Args:
            tool_name: Name of the tool being called
            kwargs: Arguments to the tool (as CaMeLValues with capabilities)
            memory_state: Current state of the system memory

        Returns:
            PolicyDecision indicating if the call is allowed
        """
        pass


def is_trusted(value: CaMeLValue) -> bool:
    """
    Check if a value comes entirely from trusted sources.

    A value is trusted if all its sources (including transitive dependencies)
    are from the User.

    Args:
        value: The CaMeLValue to check

    Returns:
        True if all sources are User sources
    """
    merged_cap = value.get_merged_capability()
    return all(src.source_type == "User" for src in merged_cap.sources)


def can_readers_read_value(
    readers: Set[str],
    value: CaMeLValue,
) -> bool:
    """
    Check if all specified readers can read the value.

    Args:
        readers: Set of principal identifiers who want to read
        value: The CaMeLValue to check access for

    Returns:
        True if all readers are allowed to read the value
    """
    merged_cap = value.get_merged_capability()

    if isinstance(merged_cap.readers, PublicSingleton):
        return True

    return readers.issubset(merged_cap.readers)


class PolicyRegistry:
    """
    Registry for managing security policies.

    Allows registering policies globally or per-tool, and provides
    methods to check all applicable policies for a tool call.
    """

    def __init__(self):
        self._policies: Dict[str, List[SecurityPolicy]] = {}
        self._global_policies: List[SecurityPolicy] = []

    def register_tool_policy(self, tool_name: str, policy: SecurityPolicy) -> None:
        """
        Register a policy for a specific tool.

        Args:
            tool_name: Name of the tool this policy applies to
            policy: The security policy to register
        """
        if tool_name not in self._policies:
            self._policies[tool_name] = []
        self._policies[tool_name].append(policy)

    def register_global_policy(self, policy: SecurityPolicy) -> None:
        """
        Register a global policy that applies to all tools.

        Args:
            policy: The security policy to register
        """
        self._global_policies.append(policy)

    def get_policies_for_tool(self, tool_name: str) -> List[SecurityPolicy]:
        """
        Get all applicable policies for a tool.

        Args:
            tool_name: Name of the tool

        Returns:
            List of policies (global + tool-specific)
        """
        return self._global_policies + self._policies.get(tool_name, [])

    def check_all(
        self,
        tool_name: str,
        kwargs: Dict[str, CaMeLValue],
        memory_state: Any,
    ) -> PolicyDecision:
        """
        Check all applicable policies for a tool call.

        Args:
            tool_name: Name of the tool being called
            kwargs: Arguments to the tool
            memory_state: Current system state

        Returns:
            First non-ALLOWED decision, or ALLOWED if all pass
        """
        for policy in self.get_policies_for_tool(tool_name):
            decision = policy.check(tool_name, kwargs, memory_state)
            if decision.result != SecurityPolicyResult.ALLOWED:
                return decision

        return PolicyDecision(SecurityPolicyResult.ALLOWED)


# ============================================================================
# Built-in Security Policies
# ============================================================================


class SendEmailPolicy(SecurityPolicy):
    """
    Security policy for the send_email tool.

    Ensures that:
    1. If recipients come from trusted source, allow
    2. Otherwise, all recipients must be able to read the email body and subject
    """

    def check(
        self,
        tool_name: str,
        kwargs: Mapping[str, CaMeLValue],
        memory_state: Any,
    ) -> PolicyDecision:
        if tool_name != "send_email":
            return PolicyDecision(SecurityPolicyResult.ALLOWED)

        recipients = kwargs.get("recipients")
        body = kwargs.get("body")
        subject = kwargs.get("subject")

        if recipients is None:
            return PolicyDecision(
                SecurityPolicyResult.DENIED,
                reason="send_email requires recipients",
            )

        # If recipients come from trusted source, allow
        if is_trusted(recipients):
            return PolicyDecision(SecurityPolicyResult.ALLOWED)

        # Check if all recipients can read the body
        recipient_set = set(recipients.raw) if isinstance(recipients.raw, list) else {recipients.raw}

        if body and not can_readers_read_value(recipient_set, body):
            return PolicyDecision(
                SecurityPolicyResult.DENIED,
                reason="Email body is not readable by all recipients",
                details={"recipients": list(recipient_set)},
            )

        if subject and not can_readers_read_value(recipient_set, subject):
            return PolicyDecision(
                SecurityPolicyResult.DENIED,
                reason="Email subject is not readable by all recipients",
                details={"recipients": list(recipient_set)},
            )

        return PolicyDecision(SecurityPolicyResult.ALLOWED)


class CreateCalendarEventPolicy(SecurityPolicy):
    """
    Security policy for creating calendar events.

    Ensures participants can read all event details.
    """

    def check(
        self,
        tool_name: str,
        kwargs: Mapping[str, CaMeLValue],
        memory_state: Any,
    ) -> PolicyDecision:
        if tool_name != "create_calendar_event":
            return PolicyDecision(SecurityPolicyResult.ALLOWED)

        participants = kwargs.get("participants")
        if participants is None:
            return PolicyDecision(SecurityPolicyResult.ALLOWED)

        # If participants come from trusted source, allow
        if is_trusted(participants):
            return PolicyDecision(SecurityPolicyResult.ALLOWED)

        participant_set = (
            set(participants.raw)
            if isinstance(participants.raw, list)
            else {participants.raw}
        )

        # Check each field is readable by participants
        fields_to_check = ["title", "description", "location", "start_time", "end_time"]

        for field_name in fields_to_check:
            field_value = kwargs.get(field_name)
            if field_value and not can_readers_read_value(participant_set, field_value):
                return PolicyDecision(
                    SecurityPolicyResult.DENIED,
                    reason=f"{field_name} is not readable by all participants",
                    details={"participants": list(participant_set)},
                )

        return PolicyDecision(SecurityPolicyResult.ALLOWED)


class SendMoneyPolicy(SecurityPolicy):
    """
    Security policy for financial transfers.

    Both recipient and amount must come from trusted sources.
    """

    def check(
        self,
        tool_name: str,
        kwargs: Mapping[str, CaMeLValue],
        memory_state: Any,
    ) -> PolicyDecision:
        if tool_name != "send_money":
            return PolicyDecision(SecurityPolicyResult.ALLOWED)

        recipient = kwargs.get("recipient")
        amount = kwargs.get("amount")

        # Both recipient and amount must come from trusted source
        if recipient and not is_trusted(recipient):
            return PolicyDecision(
                SecurityPolicyResult.DENIED,
                reason="Transfer recipient must come from trusted source",
            )

        if amount and not is_trusted(amount):
            return PolicyDecision(
                SecurityPolicyResult.DENIED,
                reason="Transfer amount must come from trusted source",
            )

        # Check for untrusted dependencies in the full graph
        for value in [recipient, amount]:
            if value is None:
                continue
            for dep in value.get_full_dependency_graph():
                if any(src.source_type == "Tool" for src in dep.capability.sources):
                    return PolicyDecision(
                        SecurityPolicyResult.DENIED,
                        reason="Transfer parameters have untrusted tool dependencies",
                    )

        return PolicyDecision(SecurityPolicyResult.ALLOWED)


class ShareFilePolicy(SecurityPolicy):
    """
    Security policy for sharing files.

    Ensures file content is readable by the share recipients.
    """

    def check(
        self,
        tool_name: str,
        kwargs: Mapping[str, CaMeLValue],
        memory_state: Any,
    ) -> PolicyDecision:
        if tool_name != "share_file":
            return PolicyDecision(SecurityPolicyResult.ALLOWED)

        share_with = kwargs.get("share_with")
        file_content = kwargs.get("file") or kwargs.get("file_id")

        if share_with is None:
            return PolicyDecision(SecurityPolicyResult.ALLOWED)

        # If share_with comes from trusted source, allow
        if is_trusted(share_with):
            return PolicyDecision(SecurityPolicyResult.ALLOWED)

        share_set = (
            set(share_with.raw)
            if isinstance(share_with.raw, list)
            else {share_with.raw}
        )

        if file_content and not can_readers_read_value(share_set, file_content):
            return PolicyDecision(
                SecurityPolicyResult.DENIED,
                reason="File content is not readable by share recipients",
            )

        return PolicyDecision(SecurityPolicyResult.ALLOWED)


# ============================================================================
# Policy Builder DSL
# ============================================================================


class PolicyBuilder:
    """
    Fluent builder for creating security policies.

    Provides a declarative way to specify policy rules.

    Example:
        policy = (
            PolicyBuilder("send_email")
            .require_readers_can_read("recipients", "body", "subject")
            .build()
        )
    """

    def __init__(self, tool_name: str):
        """
        Initialize a policy builder for a specific tool.

        Args:
            tool_name: Name of the tool this policy applies to
        """
        self._tool_name = tool_name
        self._conditions: List[Callable[[Dict[str, CaMeLValue]], Optional[str]]] = []

    def require_trusted_source(self, *param_names: str) -> "PolicyBuilder":
        """
        Require parameters to come from trusted sources.

        Args:
            param_names: Names of parameters that must be trusted

        Returns:
            Self for chaining
        """
        def check(kwargs: Dict[str, CaMeLValue]) -> Optional[str]:
            for name in param_names:
                if name in kwargs and not is_trusted(kwargs[name]):
                    return f"{name} must come from trusted source"
            return None

        self._conditions.append(check)
        return self

    def require_readers_can_read(
        self,
        readers_param: str,
        *data_params: str,
    ) -> "PolicyBuilder":
        """
        Require that readers can read the specified data.

        Args:
            readers_param: Parameter containing the readers
            data_params: Parameters containing data to check

        Returns:
            Self for chaining
        """
        def check(kwargs: Dict[str, CaMeLValue]) -> Optional[str]:
            if readers_param not in kwargs:
                return None

            readers_value = kwargs[readers_param]
            readers = (
                set(readers_value.raw)
                if isinstance(readers_value.raw, list)
                else {readers_value.raw}
            )

            for param in data_params:
                if param in kwargs:
                    if not can_readers_read_value(readers, kwargs[param]):
                        return f"{param} is not readable by {readers_param}"
            return None

        self._conditions.append(check)
        return self

    def require_no_tool_dependencies(self, *param_names: str) -> "PolicyBuilder":
        """
        Require parameters to have no tool dependencies.

        Args:
            param_names: Parameters that must not depend on tool outputs

        Returns:
            Self for chaining
        """
        def check(kwargs: Dict[str, CaMeLValue]) -> Optional[str]:
            for name in param_names:
                if name not in kwargs:
                    continue
                value = kwargs[name]
                for dep in value.get_full_dependency_graph():
                    if any(src.source_type == "Tool" for src in dep.capability.sources):
                        return f"{name} has untrusted tool dependencies"
            return None

        self._conditions.append(check)
        return self

    def custom_check(
        self,
        check_fn: Callable[[Dict[str, CaMeLValue]], Optional[str]],
    ) -> "PolicyBuilder":
        """
        Add a custom check function.

        Args:
            check_fn: Function taking kwargs and returning error message or None

        Returns:
            Self for chaining
        """
        self._conditions.append(check_fn)
        return self

    def build(self) -> SecurityPolicy:
        """
        Build the security policy.

        Returns:
            A SecurityPolicy implementing the specified checks
        """
        conditions = self._conditions
        tool_name = self._tool_name

        class BuiltPolicy(SecurityPolicy):
            def check(
                self,
                name: str,
                kwargs: Mapping[str, CaMeLValue],
                memory: Any,
            ) -> PolicyDecision:
                if name != tool_name:
                    return PolicyDecision(SecurityPolicyResult.ALLOWED)

                for condition in conditions:
                    error = condition(dict(kwargs))
                    if error:
                        return PolicyDecision(
                            SecurityPolicyResult.DENIED,
                            reason=error,
                        )

                return PolicyDecision(SecurityPolicyResult.ALLOWED)

        return BuiltPolicy()


# ============================================================================
# Default Policy Factories
# ============================================================================


def create_default_policies() -> Dict[str, SecurityPolicy]:
    """
    Create the default set of security policies.

    Returns:
        Dictionary mapping tool names to their policies
    """
    return {
        "send_email": SendEmailPolicy(),
        "create_calendar_event": CreateCalendarEventPolicy(),
        "send_money": SendMoneyPolicy(),
        "share_file": ShareFilePolicy(),
    }
