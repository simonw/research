# LLM-RLM Plugin Specification

## Overview

`llm-rlm` is an LLM plugin that implements the Recursive Language Model (RLM) pattern
as a set of tools. It allows any LLM with tool-calling support to interact with
arbitrarily large context through a sandboxed Python REPL environment (pyeryx),
with the ability to spawn sub-LLM calls for recursive reasoning.

## Core Tools

### 1. `rlm_execute_python`

**Purpose**: Execute Python code in a persistent sandboxed REPL session.

**Arguments**:
- `code: str` - Python code to execute in the sandbox

**Returns**: String containing stdout output from execution (truncated to configurable limit).

**Behavior**:
- Code runs in an `eryx.Session` that persists state across calls
- A `context` variable is pre-loaded with the user's input context
- `llm_query` and `llm_batch` are available as async callbacks
- Output from `print()` is captured and returned
- Errors are caught and returned as error messages
- Output is truncated to prevent context bloat (default: 8192 chars)

### 2. `rlm_submit_answer`

**Purpose**: Submit the final answer after analysis is complete.

**Arguments**:
- `answer: str` - The final answer text
- (OR) `variable_name: str` - Name of a variable in the REPL whose value is the answer

**Returns**: Confirmation message with the submitted answer.

**Behavior**:
- If `variable_name` is provided, retrieves the value from the REPL session
- Signals that the RLM has completed its analysis
- Equivalent to `FINAL()` / `FINAL_VAR()` in the original RLM

## Callbacks Available in Sandbox

### `llm_query(prompt: str) -> dict`

**Purpose**: Query a sub-LLM from within the sandboxed Python code.

**Usage in sandbox**:
```python
result = await llm_query(prompt="Classify this text: " + chunk)
print(result["response"])
```

**Behavior**:
- Calls the configured sub-LLM model with the given prompt
- Uses the API key from the host environment (never exposed to sandbox)
- Returns `{"response": "..."}` with the model's text response
- Subject to ResourceLimits (callback timeout)

### `llm_batch(prompts: list) -> dict`

**Purpose**: Query multiple sub-LLM calls in parallel.

**Usage in sandbox**:
```python
prompts = [f"Summarize: {chunk}" for chunk in chunks]
results = await llm_batch(prompts=prompts)
for r in results["responses"]:
    print(r)
```

**Behavior**:
- Runs all prompts concurrently against the sub-LLM
- Returns `{"responses": ["...", "...", ...]}` list of response strings
- More efficient than sequential `llm_query` calls

## Toolbox State Management

The plugin uses an `llm.Toolbox` subclass (`RLMToolbox`) that maintains:

- **eryx Session**: Persistent Python REPL with variables preserved across calls
- **Context**: The large input text stored as `context` variable in the session
- **Iteration count**: Track number of REPL executions
- **Token accounting**: Track approximate tokens used by sub-LLM calls

## System Prompt

The toolbox's `prepare()` method can inject guidance into the conversation:

```
You have access to an RLM (Recursive Language Model) environment.

The user's context is stored in a `context` variable in a Python REPL.
Use the `rlm_execute_python` tool to write Python code that explores
and analyzes this context.

Within your Python code, you have access to:
- `context` - the full input text/data
- `await llm_query(prompt)` - query a sub-LLM (returns {"response": "..."})
- `await llm_batch(prompts=[...])` - parallel sub-LLM queries

Strategies:
1. Peek at the context first (e.g., print(context[:2000]))
2. Use regex/string operations to filter relevant sections
3. Chunk the context and use llm_query/llm_batch for semantic analysis
4. Build up your answer incrementally in variables

When done, use `rlm_submit_answer` with your final answer.
```

## Configuration

### Environment Variables
- `OPENAI_API_KEY` - API key for sub-LLM calls (read from host env)

### Plugin Settings (via LLM plugin config or constructor)
- `sub_model`: Model to use for sub-LLM calls (default: same as root model)
- `max_output_chars`: Max characters of REPL output returned (default: 8192)
- `execution_timeout_ms`: Max time per code execution (default: 120000ms)
- `max_memory_bytes`: Max memory for sandbox (default: 128MB)
- `callback_timeout_ms`: Max time per sub-LLM callback (default: 60000ms)

## Dependencies

- `llm>=0.26` - LLM framework
- `pyeryx` - WebAssembly-based Python sandbox
- `openai` - For sub-LLM API calls

## Project Structure

```
llm-rlm/
├── pyproject.toml
├── llm_rlm.py          # Main plugin with RLMToolbox
├── tests/
│   ├── test_llm_rlm.py # Plugin registration and tool tests
│   └── conftest.py      # Test fixtures
└── README.md
```
