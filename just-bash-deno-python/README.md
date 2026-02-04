# just-bash: Deno JSONL Server + Python Client

Investigation into using [just-bash](https://github.com/vercel-labs/just-bash) (a sandboxed bash emulator) from Deno, wrapping it as a JSONL-over-stdio process, and building a Python client library.

## What is just-bash?

just-bash (v2.8.0) is a pure TypeScript bash emulator with:
- ~97 built-in commands (grep, sed, awk, jq, curl, find, sort, etc.)
- In-memory virtual filesystem (no real disk access)
- Optional network access (curl with configurable allow-lists)
- WASM-based sqlite3 and Python support
- Designed for AI agent sandboxing

## Running in Deno

just-bash works in Deno via its NPM package:

```typescript
import { Bash } from "npm:just-bash";

const bash = new Bash({
  network: { dangerouslyAllowFullInternetAccess: true }  // for curl
});

const result = await bash.exec("echo hello | tr a-z A-Z");
// { stdout: "HELLO\n", stderr: "", exitCode: 0 }
```

**Note**: On some systems, `DENO_TLS_CA_STORE=system` is needed for NPM registry access.

### Confirmed working features in Deno
- Basic commands: echo, cat, printf, ls, mkdir, touch, rm, cp, mv
- Pipes, redirections, command chaining (&&, ||)
- Variables, arithmetic, parameter expansion, brace expansion
- Loops (for, while), conditionals (if/else), functions
- grep, sed, awk, sort, cut, tr, wc, head, tail, seq, xargs
- jq, diff, find, tree, base64, sha256sum
- curl (GET, POST, headers) with network enabled
- html-to-markdown
- Subshells, command substitution, here documents, arrays
- Virtual filesystem persistence across exec() calls

### Known Deno-specific issues
- sqlite3 fails (SharedArrayBuffer/DataView incompatibility)
- yq fails (dynamic require of "process")

## JSONL Server (`just_bash_server.ts`)

A self-contained Deno script that turns just-bash into a long-running process communicating via newline-delimited JSON on stdin/stdout.

### Protocol

**Input** (one JSON object per line on stdin):
```json
{"id": "550e8400-e29b-41d4-a716-446655440000", "command": "echo hello"}
{"id": "uuid", "command": "curl -s https://example.com", "env": {"KEY": "val"}, "cwd": "/tmp", "timeout_ms": 5000}
```

**Output** (one JSON object per line on stdout):
```json
{"id": "550e8400-e29b-41d4-a716-446655440000", "stdout": "hello\n", "stderr": "", "exit_code": 0}
```

### Features
- **Persistent state**: files written in one request can be read in the next
- **UUID tracking**: each request `id` is reflected in the response
- **Per-request overrides**: optional `env`, `cwd`, and `timeout_ms`
- **Special commands**: `__ping`, `__shutdown`, `__reset`, `__list_files`
- **Network flag**: pass `--network` to enable curl support
- **Ready signal**: prints "READY\n" on stderr when initialized

### Running
```bash
deno run --allow-all just_bash_server.ts [--network]
```

## Python Client Library (`just_bash_py`)

A Python package providing sync and async APIs for the JSONL server.

### Sync API

```python
from just_bash_py import JustBash

with JustBash(network=True) as bash:
    # Basic command
    result = bash.run("echo hello")
    print(result.stdout)   # "hello\n"
    print(result.exit_code)  # 0
    print(result.ok)         # True

    # Persistent filesystem
    bash.run('echo "data" > /tmp/file.txt')
    result = bash.run("cat /tmp/file.txt")  # "data\n"

    # Pipelines
    result = bash.run('echo \'{"k":"v"}\' | jq -r .k')  # "v\n"

    # curl
    result = bash.run("curl -s https://httpbin.org/get | jq .url")

    # Helper methods
    bash.write_file("/tmp/hello.txt", "hello world")
    content = bash.read_file("/tmp/hello.txt")

    # Per-request env/cwd
    result = bash.run('echo "$MY_VAR"', env={"MY_VAR": "value"})

    # Reset (clear all state)
    bash.reset()
```

### Async API

```python
from just_bash_py import AsyncJustBash

async with AsyncJustBash(network=True) as bash:
    result = await bash.run("echo hello")
    await bash.write_file("/tmp/data.txt", "content")
    content = await bash.read_file("/tmp/data.txt")
    await bash.reset()
```

### BashResult

```python
@dataclass(frozen=True)
class BashResult:
    stdout: str
    stderr: str
    exit_code: int
    error: str | None = None

    @property
    def ok(self) -> bool: ...  # True if exit_code == 0
```

## Test Results

59 tests passing across sync and async suites:

```
tests/test_sync.py  - 39 tests (ping, echo, variables, pipelines, grep, jq,
                      awk, sed, tr, cut, wc, filesystem persistence, scripts,
                      loops, functions, conditionals, heredocs, brace expansion,
                      error handling, env overrides, curl GET/headers, reset,
                      base64, sha256, html-to-markdown, context manager)

tests/test_async.py - 20 tests (same coverage with async/await API)
```

Run tests with:
```bash
PATH="/root/.deno/bin:$PATH" uv run pytest tests/ -v
```

## File Structure

```
just-bash-deno-python/
├── README.md                  # This file
├── notes.md                   # Investigation notes
├── just_bash_server.ts        # Deno JSONL server
├── test_basic.ts              # Basic Deno tests
├── test_advanced.ts           # Advanced Deno tests (curl, etc.)
├── test_server.ts             # Deno JSONL server tests
├── pyproject.toml             # Python project config
├── just_bash_py/              # Python client library
│   ├── __init__.py
│   └── client.py              # JustBash + AsyncJustBash
└── tests/                     # pytest tests
    ├── __init__.py
    ├── test_sync.py           # Sync API tests
    └── test_async.py          # Async API tests
```
