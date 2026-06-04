# Monty investigation notes

Started: 2026-05-22

Goal: clone `pydantic/monty` to `/tmp`, then use `uv run --with pydantic-monty` to experimentally characterize supported Python code, sandbox behavior, memory limits, time limits, and practical usage guidance.

## 2026-05-22 initial setup

- Created this investigation folder under `/Users/simon/Dropbox/dev/research`.
- Cloned `https://github.com/pydantic/monty` to `/tmp/monty` for reference.
- `/tmp/monty` was at commit `2d43c36` (`2026-05-19T21:39:35+01:00`, "avoid UB in `Heap::dec_ref` (#451)") when inspected.
- Existing workspace has unrelated dirty files under `sqlite-wal-docker-containers`; I will not stage or modify them.
- README says Monty is experimental and intentionally limited: a small Rust Python interpreter for agent-written code, with controlled filesystem/env/network via host/external callbacks, type checking, snapshots, stdout/stderr collection, and resource limits.
- README claims supported stdlib subset includes `sys`, `os`, `typing`, `asyncio`, `re`, `datetime`, `json`, and `dataclasses`; unsupported goals include arbitrary stdlib, third-party libraries, and currently classes/match statements per README, though I need to test the actual package behavior.
- Python API exposes `Monty`, `MontyRepl`, `MountDir`, `OSAccess`, `MemoryFile`, `CallbackFile`, `CollectString`, `CollectStreams`, `FunctionSnapshot`, `FutureSnapshot`, `NameLookupSnapshot`, and `ResourceLimits`.
- Resource limit knobs: `max_allocations`, `max_duration_secs`, `max_memory`, `gc_interval`, `max_recursion_depth`.
- First live package run through `uv run --with pydantic-monty` installed `pydantic-monty` 0.0.17.

## 2026-05-22 experiment harness

- Wrote `experiments.py`, executed with `uv run --with pydantic-monty python monty-investigation-2026-05-22/experiments.py --json monty-investigation-2026-05-22/results.json`.
- Latest run covered 122 cases: 76 succeeded and 46 errored/denied.
- Supported in 0.0.17: normal arithmetic, strings, bytes literals, collections, slicing, unpacking, `for`/`while`, `try`/`except`/`finally`, recursion, default/keyword/varargs/kwargs functions, lambdas, comprehensions, generator expressions, `zip`, `map`, `filter`, `sorted(key=...)`, several builtins (`sum`, `min`, `max`, `any`, `all`, `abs`, `pow`, `divmod`, `isinstance`, `type`, `repr`, `hash`), `math`, `sys`, `re`, `json`, and basic `datetime` constructors.
- Unsupported/limited: class definitions, match statements, context managers, yield, complex constants, matrix multiplication, `eval`, `exec`, `globals`, `dir`, `locals`, `callable`, `open`, `object`; `getattr('abc', 'upper')` raised `AttributeError`, suggesting many normal Python methods are not exposed via generic attribute lookup.
- Surprising f-string finding: in pydantic-monty 0.0.17, `f'{1000:,d}'`, `f'{1000:_d}'`, and `f'{255:#x}'` all succeeded but silently omitted the grouping/alternate-form flags (`'1000'`, `'1000'`, `'ff'`). The current cloned source has Rust parser tests expecting those flags to be rejected with syntax errors, so this appears version-sensitive and likely improved after the published package I tested.
- Imports: `statistics`, `random`, `socket`, `subprocess`, and third-party `pydantic` failed with `ModuleNotFoundError`; `typing.NamedTuple` import failed, although the Python wrapper can round-trip NamedTuple inputs according to source tests.
- Sandbox: direct `open` is undefined. Filesystem/env/time style operations (`Path.exists`, `os.getenv`, `datetime.now`) require `OSAccess`, `MountDir`, or an OS callback; otherwise they fail as unsupported OS functions. `socket` is not importable even with `OSAccess`.
- `OSAccess` memory files worked for reads/writes and controlled environment variables. `MountDir` read-only blocked writes, overlay writes did not change the host, read-write mode did change the host, path traversal and unmounted paths were blocked.
- External functions are the main escape hatch: a deliberately exposed host callback could read `/etc/hosts` and return the length, so security depends on only exposing safe callbacks.
- Resource limits were strong in these probes: a `while True` loop and `sum(range(10**18))` both tripped a 50ms timeout in roughly 50ms; memory, allocation, recursion, and huge bigint pow limits also failed quickly with typed inner exceptions.
- Snapshots worked: `start()` produced a `FunctionSnapshot` for `fetch(url)`, serialized to 366 bytes, reloaded with `load_snapshot`, resumed, and completed. `Monty.dump()`/`Monty.load()` also worked.
- REPL state persisted across snippets and survived a runtime error in my test.

## 2026-05-22 finalization

- Wrote `README.md` report summarizing setup, supported Python, unsupported behavior, sandbox findings, resource limits, external calls/snapshots, type checking, and practical usage guidance.
- Validated `experiments.py` with `uv run --with pydantic-monty python -m py_compile`.
- Validated `results.json` with `python3 -m json.tool`.
- Removed generated `__pycache__` after validation so the folder only contains selected investigation artifacts.
