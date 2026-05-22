# pydantic-monty investigation

Investigation date: 2026-05-22

This folder records an experimental look at [`pydantic-monty`](https://github.com/pydantic/monty), focused on what Python it actually runs, how sandboxing behaves, and how resource limits perform in practice.

## Setup

- Cloned `pydantic/monty` to `/tmp/monty` for reference.
- Reference checkout: `2d43c36`, committed `2026-05-19T21:39:35+01:00`, `avoid UB in Heap::dec_ref (#451)`.
- Live experiments used the published package through:

```bash
uv run --with pydantic-monty python monty-investigation-2026-05-22/experiments.py --json monty-investigation-2026-05-22/results.json
```

- Tested package version: `pydantic-monty` `0.0.17`.
- Latest run: 122 cases, 76 successful, 46 errors or expected denials.

The raw results are in `results.json`. The harness is in `experiments.py`.

## High-Level Findings

Monty is useful as a small, fast, controlled interpreter for agent-authored Python, especially when the code should transform data, branch, loop, call a few explicitly exposed host tools, read/write a virtual filesystem, or pause/resume across external calls.

It is not a drop-in Python sandbox. It is deliberately much smaller than CPython. The good news is that the boundary is fairly crisp: unsupported imports, missing builtins, unmounted filesystem access, path traversal, and absent host callbacks generally fail as typed Monty errors rather than escaping into host execution.

The key security model is: Monty code cannot directly touch most host resources, but any external function or OS callback you expose is fully trusted. If you expose a callback that reads host files, Monty code can read host files through that callback.

## Python That Worked

The package handled a larger subset of everyday Python than the README headline suggests:

- Expressions: arithmetic, bools, comparisons, ternary expressions, walrus assignment, f-strings, slicing.
- Data structures: list, dict, set, tuple literals; nested structures; bytes literals; big ints.
- Assignment/control flow: unpacking, augmented assignment, `for`, `while`, `break`, `continue`, `try`/`except`/`finally`, `assert`.
- Functions: definitions, recursion, default args, keyword args, `*args`, `**kwargs`, lambdas.
- Comprehensions: list, dict, set, generator expressions, nested comprehensions.
- Builtins tested successfully: `sum`, `len`, `range`, `list`, `str`, `sorted`, `zip`, `map`, `filter`, `any`, `all`, `min`, `max`, `abs`, `pow`, `divmod`, `isinstance`, `type`, `repr`, `hash`.
- Stdlib modules tested successfully: `sys`, `math`, `re`, `json`, and basic `datetime` constructors.
- Async external calls: top-level `await` and `asyncio.gather` worked with `run_async`.
- REPL state persisted across snippets and survived a runtime error in the tested scenario.

## Python That Failed Or Was Limited

These either failed at construction time or runtime:

- Syntax/features: class definitions, `match`, `with`, `yield`, complex constants, matrix multiplication.
- Builtins/introspection: `eval`, `exec`, `globals`, `dir`, `locals`, `callable`, `open`, and `object`.
- Generic attribute lookup is limited. For example, `getattr('abc', 'upper')()` raised `AttributeError`, even though string methods like `.strip()`, `.replace()`, and `.split()` worked directly.
- `bytes([100])` failed even though bytes literals and bytes input round-tripping worked.
- Imports failed for `statistics`, `random`, `socket`, `subprocess`, and third-party `pydantic`.
- `from typing import NamedTuple` failed in Monty code, although the source tests show Python wrapper support for NamedTuple inputs.
- `datetime.datetime.now()`, `os.getenv`, `os.environ`, and `Path.exists()` failed without an OS provider because they are treated as controlled OS calls.

One version-sensitive surprise: in the tested `0.0.17` package, f-string flags like `f'{1000:,d}'`, `f'{1000:_d}'`, and `f'{255:#x}'` succeeded but silently omitted grouping/alternate-form behavior. The cloned source already has parser tests expecting these flags to be rejected, so newer source may be stricter than the published wheel.

## Sandbox Findings

Direct host access was blocked or absent in the tested paths:

- `open('/etc/passwd')` failed because `open` is undefined.
- `Path('/etc/passwd').exists()` failed without an OS provider.
- `os.getenv('HOME')` failed without an OS provider.
- `socket` was not importable, including while using `OSAccess`.
- Third-party packages were not importable from Monty code.

Controlled access worked as designed:

- `OSAccess` served in-memory files and controlled environment variables.
- `MountDir('/mnt', ..., mode='read-only')` allowed reads and blocked writes.
- `MountDir(..., mode='overlay')` allowed writes visible to Monty without changing host files.
- `MountDir(..., mode='read-write')` changed the host directory.
- Path traversal through a mount was blocked with `PermissionError`.
- Access to unmounted paths was blocked with `PermissionError`.

The escape hatch is explicit host functionality. A test callback named `host_read_len` read `/etc/hosts` from the host and returned its length. That is expected: callbacks are outside the sandbox and must be treated as the capability boundary.

## Resource Limits

The limit probes were strong and fast on this machine:

| Case | Limit | Result |
| --- | --- | --- |
| `while True: pass` | `max_duration_secs=0.05` | `TimeoutError` in about 50ms |
| `sum(range(10**18))` | `max_duration_secs=0.05` | `TimeoutError` in about 50ms |
| Growing list of strings | `max_memory=100` | `MemoryError` immediately |
| Many list allocations | `max_allocations=10` | `MemoryError` immediately |
| Recursive function | `max_recursion_depth=5` | `RecursionError` immediately |
| `2 ** 10000000` | `max_memory=1_000_000` | `MemoryError` immediately |
| `sum(range(1000))` | combined normal limits | succeeded |

The important detail is that the timeout also worked inside a Rust-side builtin loop (`sum(range(10**18))`), not just bytecode-level Python loops.

## External Calls And Snapshots

External functions worked for normal calls and exception propagation:

- `host_add(2, 3)` returned `5`.
- A host `ValueError` was catchable inside Monty code.
- A missing external function became a Monty-wrapped `NameError`.

Snapshots worked:

- `Monty.start()` paused at `fetch(url)` as a `FunctionSnapshot`.
- The snapshot serialized to 366 bytes.
- `load_snapshot()` restored it.
- Resuming with `{'return_value': 'abcdef'}` completed with output `6`.

Compiled `Monty` instances also serialized and loaded successfully with `dump()` and `Monty.load()`.

## Type Checking

Monty's type checker caught:

- Unsupported operator: `"hello" + 1`.
- Invalid return type: `def f() -> int: return 'x'`.
- Undefined names.
- Stub mismatch: calling a stubbed `external(x: int)` with a string.

Type-check stubs worked for declaring external functions or input-like names before checking.

## Practical Usage Guidance

Use Monty for:

- Letting an agent write small data-processing programs instead of large tool-call DAGs.
- Deterministic transformations over JSON-like data.
- Sandboxed reasoning code that can call a small set of explicitly exposed host functions.
- Async workflows where Monty can `await` host calls or pause/resume through snapshots.
- Virtual file workflows through `OSAccess` or `MountDir`, especially overlay mode.

Avoid or be careful with:

- Code that expects CPython's full stdlib or installed packages.
- Code that defines classes, uses context managers, generators, pattern matching, or dynamic introspection.
- Broad host callbacks like `read_file`, `request_url`, `run_shell`, or generic OS dispatchers unless the agent is meant to have that exact authority.
- Relying on subtle formatting behavior without tests. The f-string flag behavior differed from current source expectations.

Recommended defaults for agent code:

- Always set `max_duration_secs`, `max_memory`, `max_allocations`, and `max_recursion_depth`.
- Prefer `OSAccess` memory files or `MountDir(..., mode='overlay')` for scratch file access.
- Keep external functions narrow and typed.
- Use type-check stubs for exposed callbacks.
- Treat `results.json` style probes as regression tests when upgrading `pydantic-monty`.

