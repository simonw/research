"""
Configuration system for CaMeL.

This module provides:
- Configuration data classes
- YAML configuration loading
- Validation of configuration
"""

from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from .policies import SecurityPolicy


class InterpreterMode(Enum):
    """Interpreter execution modes."""
    NORMAL = "normal"   # Standard dependency tracking
    STRICT = "strict"   # Adds control flow dependencies


@dataclass
class LLMConfig:
    """Configuration for an LLM."""
    model_name: str
    api_key: Optional[str] = None
    api_base: Optional[str] = None
    temperature: float = 0.0
    max_tokens: int = 4096

    def __post_init__(self):
        """Validate configuration."""
        if self.temperature < 0 or self.temperature > 2:
            raise ValueError("temperature must be between 0 and 2")
        if self.max_tokens < 1:
            raise ValueError("max_tokens must be positive")


@dataclass
class CaMeLConfig:
    """Complete CaMeL configuration."""

    # LLM configurations
    p_llm: LLMConfig
    q_llm: LLMConfig  # Can be same as p_llm or different/cheaper

    # Interpreter settings
    mode: InterpreterMode = InterpreterMode.NORMAL
    max_retries: int = 10
    max_iterations: int = 100  # Maximum loop iterations

    # Security settings
    require_user_confirmation_on_deny: bool = True
    audit_logging: bool = True

    # Policy configuration
    enabled_policies: List[str] = field(default_factory=list)
    custom_policies: Dict[str, "SecurityPolicy"] = field(default_factory=dict)

    # Tool configuration
    enabled_tools: List[str] = field(default_factory=list)
    tool_timeout_seconds: float = 30.0

    # System context
    user_name: Optional[str] = None
    user_email: Optional[str] = None
    company_name: Optional[str] = None

    def __post_init__(self):
        """Validate configuration."""
        if self.max_retries < 0:
            raise ValueError("max_retries must be non-negative")
        if self.max_iterations < 1:
            raise ValueError("max_iterations must be positive")
        if self.tool_timeout_seconds <= 0:
            raise ValueError("tool_timeout_seconds must be positive")


def load_config(config_path: str) -> CaMeLConfig:
    """
    Load configuration from a YAML file.

    Args:
        config_path: Path to the YAML configuration file

    Returns:
        CaMeLConfig instance

    Raises:
        FileNotFoundError: If config file doesn't exist
        ValueError: If configuration is invalid
    """
    path = Path(config_path)
    if not path.exists():
        raise FileNotFoundError(f"Configuration file not found: {config_path}")

    try:
        import yaml
    except ImportError:
        raise ImportError("PyYAML required for loading config files. Install with: pip install pyyaml")

    with open(path) as f:
        data = yaml.safe_load(f)

    # Parse LLM configs
    p_llm_data = data.get("p_llm", {})
    q_llm_data = data.get("q_llm", {})

    p_llm = LLMConfig(
        model_name=p_llm_data.get("model_name", "claude-sonnet-4-20250514"),
        api_key=p_llm_data.get("api_key"),
        api_base=p_llm_data.get("api_base"),
        temperature=p_llm_data.get("temperature", 0.0),
        max_tokens=p_llm_data.get("max_tokens", 4096),
    )

    q_llm = LLMConfig(
        model_name=q_llm_data.get("model_name", "claude-3-5-haiku-20241022"),
        api_key=q_llm_data.get("api_key"),
        api_base=q_llm_data.get("api_base"),
        temperature=q_llm_data.get("temperature", 0.0),
        max_tokens=q_llm_data.get("max_tokens", 1024),
    )

    # Parse mode
    mode_str = data.get("mode", "normal")
    mode = InterpreterMode(mode_str)

    return CaMeLConfig(
        p_llm=p_llm,
        q_llm=q_llm,
        mode=mode,
        max_retries=data.get("max_retries", 10),
        max_iterations=data.get("max_iterations", 100),
        require_user_confirmation_on_deny=data.get("require_user_confirmation_on_deny", True),
        audit_logging=data.get("audit_logging", True),
        enabled_policies=data.get("enabled_policies", []),
        enabled_tools=data.get("enabled_tools", []),
        tool_timeout_seconds=data.get("tool_timeout_seconds", 30.0),
        user_name=data.get("user_name"),
        user_email=data.get("user_email"),
        company_name=data.get("company_name"),
    )


def create_default_config() -> CaMeLConfig:
    """
    Create a default configuration.

    Returns:
        CaMeLConfig with sensible defaults
    """
    return CaMeLConfig(
        p_llm=LLMConfig(model_name="claude-sonnet-4-20250514"),
        q_llm=LLMConfig(model_name="claude-3-5-haiku-20241022"),
        mode=InterpreterMode.NORMAL,
        max_retries=10,
        enabled_policies=["send_email", "create_calendar_event", "send_money"],
        enabled_tools=[
            "send_email",
            "get_received_emails",
            "search_emails",
            "get_file_by_id",
            "search_files",
            "create_calendar_event",
            "get_day_calendar_events",
        ],
    )


def save_config(config: CaMeLConfig, config_path: str) -> None:
    """
    Save configuration to a YAML file.

    Args:
        config: CaMeLConfig to save
        config_path: Path to save the configuration
    """
    try:
        import yaml
    except ImportError:
        raise ImportError("PyYAML required for saving config files. Install with: pip install pyyaml")

    data = {
        "p_llm": {
            "model_name": config.p_llm.model_name,
            "temperature": config.p_llm.temperature,
            "max_tokens": config.p_llm.max_tokens,
        },
        "q_llm": {
            "model_name": config.q_llm.model_name,
            "temperature": config.q_llm.temperature,
            "max_tokens": config.q_llm.max_tokens,
        },
        "mode": config.mode.value,
        "max_retries": config.max_retries,
        "max_iterations": config.max_iterations,
        "require_user_confirmation_on_deny": config.require_user_confirmation_on_deny,
        "audit_logging": config.audit_logging,
        "enabled_policies": config.enabled_policies,
        "enabled_tools": config.enabled_tools,
        "tool_timeout_seconds": config.tool_timeout_seconds,
    }

    if config.user_name:
        data["user_name"] = config.user_name
    if config.user_email:
        data["user_email"] = config.user_email
    if config.company_name:
        data["company_name"] = config.company_name

    with open(config_path, "w") as f:
        yaml.dump(data, f, default_flow_style=False, sort_keys=False)
