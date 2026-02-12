# llm-rlm

Recursive Language Model (RLM) tools for [LLM](https://llm.datasette.io/) - process
unbounded context through a sandboxed Python REPL without context rot.

## What is an RLM?

A Recursive Language Model stores large context as a Python variable in a sandboxed
REPL rather than feeding it directly into the LLM's context window. The LLM writes
Python code to explore, filter, and transform the context, and can spawn sub-LLM
calls for recursive reasoning. This avoids context rot and enables processing of
essentially unlimited input lengths.

Based on [Zhang & Khattab (2025)](https://arxiv.org/abs/2512.24601v1).

## Installation

Requires Python 3.12+.

```bash
llm install llm-rlm
```

## Usage

### Python API

```python
import llm
from llm_rlm import RLMToolbox

# Load your large context (never sent directly to the model)
with open("huge_logfile.txt") as f:
    context = f.read()

toolbox = RLMToolbox(context=context, sub_model="gpt-5.2")
model = llm.get_model("gpt-5.2")

response = model.chain(
    "Find and count all ERROR entries in the context. "
    "The context is in the `context` variable in the sandbox.",
    tools=[toolbox],
)
print(response.text())
```

### CLI

```bash
# Use the RLM toolbox with a prompt
llm -T RLMToolbox "Search the context for patterns" --td
```

### Tools Provided

The `RLMToolbox` exposes two tools:

- **`RLMToolbox_execute_python(code)`** - Execute Python code in the sandbox.
  The `context` variable contains your input. Use `print()` for output.
  `await llm_query(prompt='...')` and `await llm_batch(prompts=[...])` spawn sub-LLM calls.

- **`RLMToolbox_submit_answer(answer=..., variable_name=...)`** - Submit the final answer,
  either directly or by referencing a variable in the sandbox session.

### Sandbox Callbacks

Within the Python sandbox, these async functions are available:

```python
# Single sub-LLM query
result = await llm_query(prompt="Classify this text: " + chunk)
print(result["response"])

# Batch sub-LLM queries
results = await llm_batch(prompts=["Summarize: " + c for c in chunks])
for r in results["responses"]:
    print(r)
```

The API key stays on the host - sandbox code never sees credentials.

## Configuration

```python
RLMToolbox(
    context="...",              # Large text to analyze
    sub_model="gpt-5.2",       # Model for sub-LLM calls
    max_output_chars=8192,      # Truncate REPL output
    execution_timeout_ms=120000 # Per-execution timeout
)
```

## Development

```bash
cd llm-rlm
uv run pytest
```
