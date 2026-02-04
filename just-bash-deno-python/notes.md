# just-bash Deno + Python Investigation Notes

## Initial Exploration

- Cloned https://github.com/vercel-labs/just-bash to /tmp/just-bash
- Version 2.8.0, Apache-2.0 license
- Pure TypeScript bash emulator with in-memory virtual filesystem
- Available on NPM as `just-bash`
- ~97 built-in commands including sed, awk, jq, grep, curl, sqlite3, python3
- Multiple filesystem backends: InMemoryFs, OverlayFs, ReadWriteFs, MountableFs
- Network access (curl) disabled by default, must be explicitly enabled
- Security model: no real filesystem access, no binary execution, WASM-based interpreters

## Key API

```typescript
const bash = new Bash({
  files: { "/path/file": "content" },
  env: { KEY: "value" },
  cwd: "/home/user",
  network: { dangerouslyAllowFullInternetAccess: true } // for curl
});

const result = await bash.exec("echo hello");
// { stdout, stderr, exitCode, env }
```

## Deno Testing Notes

### Deno version: 2.6.8

### Need DENO_TLS_CA_STORE=system for NPM registry access

### Working features (confirmed in Deno):
- Basic commands: echo, echo -e, cat, printf, ls, mkdir, touch, rm, cp, mv
- Pipes, redirections (>, >>, 2>, 2>&1)
- Variables, arithmetic ($((...))), parameter expansion (${var%.txt}, ${var##*.})
- Brace expansion ({a,b,c}{1,2})
- Loops (for), conditionals (if/else), functions
- grep, sed, awk, sort, cut, tr, wc, head, tail, seq
- jq (JSON processing), xargs, diff, find, tree, du
- sha256sum, base64
- tar (create)
- **curl** - GET, POST, custom headers all work with `network: { dangerouslyAllowFullInternetAccess: true }`
- html-to-markdown (pipe HTML in, get markdown out)
- Subshells, command substitution $(...)
- Here documents (<<EOF)
- Arrays (${arr[1]}, ${#arr[@]})
- Virtual filesystem persistence across exec() calls on same Bash instance
- Exit codes, && and || chaining

### Issues in Deno:
- sqlite3: Worker fails with DataView/SharedArrayBuffer error (Deno compatibility issue)
- yq: "Dynamic require of 'process' is not supported" (CJS/ESM module issue)
- Env vars do NOT persist across separate exec() calls (each exec gets fresh env unless using bash.getEnv())
- curl POST with -d doesn't populate form data in httpbin (may be content-type related)
- du reports 0 for all directories (virtual FS doesn't track real sizes)

### curl details:
- `curl -s URL` works for GET
- `curl -s URL | jq .field` pipeline works
- `curl -s -H "Header: value" URL` works
- POST works but form parsing may differ from real curl

## JSONL Server

Built `just_bash_server.ts` - a Deno script that:
- Reads newline-delimited JSON from stdin
- Executes commands via a persistent `Bash` instance
- Returns results as newline-delimited JSON on stdout
- Each request has `id` (UUID) reflected in response
- Filesystem state persists across requests
- Supports: `--network` flag for curl access
- Special commands: `__ping`, `__shutdown`, `__reset`, `__list_files`
- Optional per-request `env`, `cwd`, `timeout_ms` overrides
- Signals "READY" on stderr when initialization is complete

All 22 test checks pass:
- Ping, basic echo, file persistence, pipelines, jq, error handling
- Complex scripts, multi-step filesystem persistence, env vars
- Per-request env overrides, sed, awk, curl, reset, timeout

## Python Library Design

Will create a Python package that:
- Spawns the Deno JSONL server as a subprocess
- Provides sync and async APIs
- Handles UUID generation and request/response matching
- Manages process lifecycle (start, stop, context manager)
