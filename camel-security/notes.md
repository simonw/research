# CaMeL Security Implementation Notes

## Project Overview
Implementing the CaMeL (Capabilities for Machine Learning) system - a defense against prompt injection attacks in LLM-based agentic systems.

Based on: "Defeating Prompt Injections by Design" (Debenedetti et al., 2025)

## Key Design Goals
1. Separate control and data flows using a Dual-LLM pattern
2. Track data provenance via a custom Python interpreter
3. Enforce security policies based on fine-grained capabilities
4. Prevent unauthorized data exfiltration through capability-based access control

## Implementation Completed

### Phase 1: Core Data Types ✓
- [x] CaMeLValue wrapper class - Generic wrapper tracking raw values with capabilities
- [x] Capability and DataSource classes - Track sources and readers
- [x] PublicSingleton for unrestricted readers
- [x] CapabilityAssigner factory methods

### Phase 2: Interpreter ✓
- [x] AST validation for restricted Python subset
- [x] Expression evaluation with capability tracking
- [x] Data flow graph implementation
- [x] Control flow handling (if/for) with STRICT mode support
- [x] List/dict/set comprehensions
- [x] F-string evaluation
- [x] Class definition support (for Pydantic models)
- [x] Maximum iteration limits for safety

### Phase 3: Security Policies ✓
- [x] Policy framework (SecurityPolicy base class, PolicyRegistry)
- [x] SendEmailPolicy - Checks recipients can read body
- [x] CreateCalendarEventPolicy - Checks participants can read event details
- [x] SendMoneyPolicy - Requires trusted source for recipient and amount
- [x] ShareFilePolicy - Checks share recipients can read file
- [x] PolicyBuilder DSL for creating policies declaratively

### Phase 4: Tool System ✓
- [x] ToolDefinition and ToolSignature classes
- [x] ToolRegistry with decorator-based registration
- [x] Capability annotators for tool outputs
- [x] Type hint to JSON Schema conversion

### Phase 5: LLM Interfaces ✓
- [x] PrivilegedLLM abstract class
- [x] QuarantinedLLM abstract class
- [x] AnthropicPLLM - Anthropic Claude implementation for P-LLM
- [x] AnthropicQLLM - Anthropic Claude implementation for Q-LLM
- [x] MockPLLM and MockQLLM for testing

### Phase 6: Main Agent ✓
- [x] CaMeLAgent orchestration class
- [x] Error handling with retry logic and error redaction
- [x] ExecutionResult for detailed execution info
- [x] Configuration system with YAML support

### Phase 7: Testing ✓
- [x] Unit tests for types (test_types.py)
- [x] Unit tests for interpreter (test_interpreter.py)
- [x] Security policy tests (test_policies.py)
- [x] Example usage (examples/basic_usage.py)

---

## Key Implementation Decisions

### 1. Capability Merging Strategy
- **Sources**: Union (data comes from all original sources)
- **Readers**: Intersection (only those who can read ALL inputs can read output)

This is critical for security - if data is combined from multiple sources, the result should only be readable by principals who could read all inputs.

### 2. NORMAL vs STRICT Mode
- **NORMAL**: Standard dependency tracking through data flow
- **STRICT**: Adds control flow dependencies (variables assigned in if/for blocks inherit dependencies from conditions)

STRICT mode prevents side-channel attacks where information leaks through control flow choices.

### 3. Error Redaction
When errors come from untrusted sources (e.g., tool outputs, Q-LLM), the error message is redacted before showing to P-LLM. This prevents prompt injection via error messages.

### 4. Python Subset Restrictions
Forbidden constructs:
- while loops (potential infinite loops)
- Generator expressions (can be infinite)
- Imports (security risk)
- eval/exec (code injection)
- break/continue (complexity)
- Function definitions (def/lambda) outside class definitions
- Side-effect methods on built-in types

### 5. Dual LLM Architecture
- **P-LLM (Privileged)**: Only sees trusted user query, generates Python code
- **Q-LLM (Quarantined)**: Parses untrusted data, NO tool access

This separation ensures P-LLM never sees potentially malicious content that could hijack the agent.

---

## File Structure

```
camel-security/
├── camel/
│   ├── __init__.py      # Package exports
│   ├── types.py         # CaMeLValue, Capability, DataSource
│   ├── errors.py        # Exception classes
│   ├── policies.py      # Security policies
│   ├── tools.py         # Tool registry
│   ├── llm.py           # LLM interfaces
│   ├── interpreter.py   # CaMeL interpreter
│   ├── agent.py         # Main agent
│   └── config.py        # Configuration
├── tests/
│   ├── __init__.py
│   ├── test_types.py
│   ├── test_interpreter.py
│   └── test_policies.py
├── examples/
│   └── basic_usage.py
├── pyproject.toml
├── notes.md
└── README.md
```

---

## Security Properties Achieved

1. **Control Flow Integrity**: P-LLM generates code without seeing untrusted data
2. **Data Flow Tracking**: All values tagged with provenance
3. **Access Control**: Recipients must be able to read what they receive
4. **Trust Boundaries**: Clear separation between trusted (User) and untrusted (Tool/Q-LLM) sources
5. **Side Channel Mitigation**: STRICT mode prevents control flow leaks

---

## Testing Notes

Run tests with:
```bash
cd camel-security
pip install -e ".[dev]"
pytest tests/ -v
```

Run examples:
```bash
python examples/basic_usage.py
```

---

## Limitations and Future Work

1. **Text-to-text attacks**: Cannot prevent attacks that only change displayed text
2. **Timing side channels**: Not addressed in current implementation
3. **User fatigue**: Frequent policy denials may lead to approval fatigue
4. **Tool annotation completeness**: Requires tools to properly annotate outputs
5. **Complex Python features**: Some advanced Python features not supported
