# LLM RLM Plugin Investigation

## Summary

This investigation researches the Recursive Language Model (RLM) pattern and implements
it as an LLM plugin (`llm-rlm`) that provides sandboxed Python REPL tools for processing
unbounded context without context rot.

## What is an RLM?

Recursive Language Models (Zhang & Khattab, MIT CSAIL, 2025) are an inference strategy
where an LLM interacts with a Python REPL environment that stores potentially huge context
as a variable. Instead of feeding millions of tokens directly into the context window
(causing "context rot" - degraded performance), the LLM:

1. Writes Python code to peek at, grep, filter, and chunk the context
2. Uses `llm_query()` to spawn sub-LLM calls on context chunks
3. Aggregates results and submits a final answer

Key results from the original paper:
- RLM(GPT-5-mini) outperforms GPT-5 by 114% on OOLONG long-context benchmark
- Performance doesn't degrade at 10M+ tokens
- Cheaper per query than feeding full context to a frontier model

## Architecture

The `llm-rlm` plugin implements the RLM pattern using:

- **LLM Toolbox**: `RLMToolbox` extends `llm.Toolbox` to provide tools with persistent state
- **pyeryx sandbox**: WebAssembly-based Python sandbox (CPython 3.14 in WASI) for
  complete isolation. Context stored as `context` variable, never in model context window
- **Host callbacks**: `llm_query` and `llm_batch` are pyeryx callbacks that run on the
  host, routing through LLM's model abstraction. API keys never enter the sandbox
- **LLM's tool loop**: Plugin provides tools; LLM's built-in tool-calling loop handles
  iteration until the model calls `submit_answer`

### Tools

| Tool | Description |
|------|-------------|
| `RLMToolbox_execute_python(code)` | Execute Python in the sandboxed REPL session |
| `RLMToolbox_submit_answer(answer/variable_name)` | Submit the final answer |

### Sandbox Callbacks

| Callback | Description |
|----------|-------------|
| `await llm_query(prompt='...')` | Single sub-LLM query, returns `{"response": "..."}` |
| `await llm_batch(prompts=[...])` | Batch sub-LLM queries, returns `{"responses": [...]}` |

## Key Design Decisions

1. **Context isolation**: Context is NEVER in the system prompt or model context window.
   It exists only as a Python variable in the pyeryx sandbox, accessible only through
   `execute_python` tool calls.

2. **Sub-LLM via LLM abstraction**: Sub-calls route through `llm.get_model()` rather than
   direct API calls, supporting any LLM backend.

3. **pyeryx for sandboxing**: Chosen over raw `exec()` (like rlm-minimal uses) for
   complete WebAssembly isolation, resource limits, and the callback mechanism that
   keeps API keys on the host side.

## Files

- `notes.md` - Research notes and implementation journal
- `specification.md` - Detailed tool specification
- `llm-rlm/` - The plugin itself
  - `llm_rlm.py` - Plugin implementation (RLMToolbox class)
  - `tests/test_llm_rlm.py` - 18 unit tests
  - `tests/test_integration.py` - Integration tests using gpt-5.2
  - `pyproject.toml` - Package configuration
  - `README.md` - Plugin documentation

## Sources

- [Recursive Language Models blog post](https://alexzhang13.github.io/blog/2025/rlm/) - Alex Zhang
- [The Potential of RLMs](https://www.dbreunig.com/2026/02/09/the-potential-of-rlms.html) - Drew Breunig
- [RLM: the paradigm of 2026](https://www.primeintellect.ai/blog/rlm) - Prime Intellect
- [rlm-minimal reference implementation](https://github.com/alexzhang13/rlm-minimal)
- [pyeryx documentation](https://github.com/eryx-org/eryx)
- [LLM tool plugins documentation](https://llm.datasette.io/en/stable/plugins/plugin-utilities.html)
