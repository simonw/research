# LLM RLM Plugin - Research Notes

## Research Phase

### What is the RLM Pattern?

Recursive Language Models (RLMs) are an inference strategy where LLMs can decompose and
recursively interact with input context of unbounded length through REPL environments.

**Key paper**: Zhang & Khattab, MIT CSAIL, Oct 2025
- Paper: https://arxiv.org/abs/2512.24601v1
- Code: https://github.com/alexzhang13/rlm
- Minimal: https://github.com/alexzhang13/rlm-minimal

**Core idea**: Instead of feeding huge context directly to an LLM (causing context rot),
the RLM:
1. Stores context as a variable in a Python REPL environment
2. Lets the LLM write Python code to explore/transform/filter the context
3. Provides `llm_query()` to spawn sub-LLM calls for recursive reasoning
4. The LLM signals completion with `FINAL(answer)` or `FINAL_VAR(variable_name)`

### Key Components Identified

From studying the gist sources (original blog, Drew Breunig, Prime Intellect):

1. **Python REPL environment** - persistent session where context is a variable
2. **llm_query(prompt)** - calls a sub-LLM from within the REPL
3. **llm_batch(prompts)** - parallel sub-LLM calls (Prime Intellect extension)
4. **FINAL(answer)** / **FINAL_VAR(var)** - answer signaling
5. **Iteration budget** - max number of REPL interactions
6. **Output truncation** - REPL output limited to prevent context bloat

### Strategies RLMs Discover

From the blog post observations:
- **Peeking**: Looking at first N characters of context
- **Grepping**: Using regex to narrow search space
- **Partition + Map**: Chunking context, running sub-LLM per chunk
- **Summarization**: Sub-LLMs summarize context chunks
- **Programmatic processing**: Using code for deterministic operations

### LLM Plugin System

- Plugins register tools via `@llm.hookimpl` and `register_tools(register)` hook
- Tool functions take typed args, return strings
- `llm.Toolbox` classes maintain state across tool calls
- Entry points in pyproject.toml under `[project.entry-points.llm]`
- Tools are invoked with `-T tool_name` on CLI

### pyeryx (Eryx Python Sandbox)

- `pip install pyeryx`, import as `eryx`
- `eryx.Session()` for persistent state (REPL-like)
- `eryx.Sandbox()` for one-off execution
- **Callbacks**: Expose host functions to sandboxed code as `await callback_name(...)`
  - JSON-serializable args/returns
  - Perfect for `llm_query` - keeps API key on host side
- **ResourceLimits**: timeout, memory, callback limits
- Sessions maintain variables across `execute()` calls
- CPython 3.14 in WebAssembly (WASI)

### rlm-minimal Reference Implementation

Studied `/tmp/rlm-minimal/`:
- `REPLEnv` class wraps Python `exec()` with restricted builtins
- Context loaded as `context` variable (string or JSON)
- `llm_query(prompt)` registered in REPL globals
- Code blocks extracted from model output via regex (```repl ... ```)
- FINAL/FINAL_VAR detected via regex patterns
- Max iterations loop with message history accumulation
- Output truncated to 100k chars

### Architecture Decision: Tools vs Code Blocks

The original RLM uses code blocks in model output. LLM's tool system uses structured
tool calls. For the plugin, we'll use LLM's Toolbox pattern:

- `execute_python(code)` tool - runs code in pyeryx Session
- Within the sandbox, `llm_query` available as async callback
- Toolbox maintains eryx Session state across calls
- Context loaded via system prompt or initial tool call

This maps naturally: the LLM calls `execute_python` (tool call) instead of outputting
code blocks (which would require custom parsing outside the LLM framework).

### Token Budget Awareness

Task specifies max 2M tokens for the entire project using gpt-5.2.
Need to be careful with:
- Number of REPL iterations
- Size of context passed to sub-LLMs
- Output truncation settings

## Implementation Phase

### Scaffold

Used `uvx cookiecutter` with `/tmp/llm-plugin-tools` template to create `llm-rlm` plugin.
Plugin located at `llm-rlm-plugin/llm-rlm/`.

### pyeryx Build Issue

pyeryx only ships pre-built wheels for cp312-abi3 (Python 3.12+). Our environment has
Python 3.11 as default but 3.12 is available. Updated `requires-python = ">=3.12"` in
pyproject.toml and used `uv run --python 3.12` for development.

### pyeryx String Quoting Bug

pyeryx wraps code in triple double quotes internally (`"""..."""`). If user code ends
with a double quote character, it creates invalid syntax like `42""""`. Workaround:
always ensure code ends with a newline (`\n`) before passing to `session.execute()`.
Implemented as `_safe_execute()` helper method.

### Design Decisions

1. **Toolbox vs Functions**: Used `llm.Toolbox` subclass for state management across
   tool calls (eryx Session persistence). Tools are `RLMToolbox_execute_python` and
   `RLMToolbox_submit_answer`.

2. **Context storage**: Context NEVER goes into the system prompt or model context.
   It's stored exclusively as a Python variable in the pyeryx sandbox, accessible
   only via `execute_python` tool calls.

3. **Sub-LLM routing**: Uses LLM's own `llm.get_model()` abstraction. The API key
   stays on the host side via pyeryx callbacks - sandbox code calls
   `await llm_query(prompt='...')` which invokes a host-side callback.

4. **LLM tool loop**: Plugin provides tools only. LLM's built-in tool-calling loop
   handles iteration - model calls execute_python repeatedly until it calls
   submit_answer.

### TDD Process

- Wrote 18 unit tests covering: registration, tool methods, execute_python behavior,
  state persistence, context isolation, error handling, output truncation, submit_answer,
  llm_query/llm_batch callback availability, and context-never-in-prompt verification.
- 17/18 passed on first implementation. Fixed 1 failure (pyeryx string quoting bug).
- All 18 pass after the fix.

### Integration Tests with gpt-5.2

Tested three scenarios:
1. **Small needle-in-haystack** (4 lines): Model finds "magic number is 42" in one call.
2. **Large needle-in-haystack** (1000 lines): Model uses regex search, finds 87654.
3. **Semantic classification** (30 items): Model counts entries describing cats by
   searching for keyword "feline", correctly returns 10.

All integration tests pass with gpt-5.2.

### CLI Exercise

Verified plugin works via `llm` CLI:

```
$ uv run llm plugins  # Shows llm-rlm with register_tools hook
$ uv run llm tools    # Shows RLMToolbox_execute_python and RLMToolbox_submit_answer
```

Tested with `llm -m gpt-5.2 -T RLMToolbox --td`:
1. **Sum of first 100 primes**: Model tried sympy (unavailable in sandbox),
   then wrote its own is_prime function. Correctly computed 24133.
2. **Context search**: Loaded 200-line log file as context. Model used
   regex/grep in sandbox to find WARNING entry and extract secret password.

Both tasks completed successfully - the model uses tool calls naturally and
adapts when sandbox doesn't have expected libraries.
