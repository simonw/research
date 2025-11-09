# Investigation Notes: OpenAI Codex Sandboxing

## Initial Exploration

Starting investigation of https://github.com/openai/codex sandboxing implementation.

### Repository Structure

Main directories:
- `codex-rs/` - Rust implementation
- `codex-rs/core/` - Core sandbox logic
- `codex-rs/linux-sandbox/` - Linux-specific sandbox implementation
- `codex-rs/windows-sandbox-rs/` - Windows sandbox (experimental)
- `docs/` - Documentation including sandbox.md

## Key Findings

### High-Level Architecture

The Codex CLI implements a multi-layered sandbox that:
1. Restricts filesystem access (read-only or workspace-write modes)
2. Blocks/allows network access
3. Uses OS-specific sandboxing mechanisms (Seatbelt on macOS, Landlock+seccomp on Linux)
4. Provides approval workflows for risky operations

### Sandbox Modes (from protocol.rs)

1. **DangerFullAccess** - No restrictions
2. **ReadOnly** - Read-only access to entire filesystem
3. **WorkspaceWrite** - Read-only + write access to workspace/cwd
   - Can specify additional writable_roots
   - Optional network_access (default: false)
   - Excludes TMPDIR and /tmp by default (configurable)
   - Special handling: .git folders are READ-ONLY even in writable workspaces

### macOS Implementation (Seatbelt)

**File**: `codex-rs/core/src/seatbelt.rs`

Uses Apple's `sandbox-exec` command with custom Scheme-like policy files:
- Base policy: `seatbelt_base_policy.sbpl`
- Network policy: `seatbelt_network_policy.sbpl`

**Key mechanisms**:
1. Invokes `/usr/bin/sandbox-exec` (hardcoded path for security)
2. Builds policy dynamically based on SandboxPolicy
3. Uses parameterized policies with `-D` flags for paths
4. Canonicalizes paths to handle symlinks (/var vs /private/var)

**Base policy allows**:
- Process forking/exec
- User preferences read
- Specific sysctls (hw.*, kern.*, vm.loadavg)
- IPC POSIX semaphores (for Python multiprocessing)
- Reading from filesystem (when full_disk_read enabled)

**Base policy denies**:
- Everything by default (deny default)
- All file writes except explicitly allowed paths

**Write access handling**:
- Full access: `(allow file-write* (regex #"^/"))`
- Workspace write: Builds `(subpath (param "WRITABLE_ROOT_N"))` rules
- .git protection: Uses `(require-not (subpath ...))` to exclude .git folders
- Example: `(require-all (subpath "workspace") (require-not (subpath ".git")))`

**Network policy** (when enabled):
- Allows network-outbound, network-inbound, system-socket
- Allows mach-lookup for security/DNS services
- Allows writes to DARWIN_USER_CACHE_DIR

### Linux Implementation (Landlock + seccomp)

**Files**:
- `codex-rs/core/src/landlock.rs`
- `codex-rs/linux-sandbox/src/landlock.rs`
- `codex-rs/linux-sandbox/src/linux_run_main.rs`

Uses two Linux kernel features:
1. **Landlock** - Filesystem access control (LSM - Linux Security Module)
2. **seccomp** - System call filtering

**Architecture**:
- Separate binary: `codex-linux-sandbox`
- Policy passed as JSON to the binary
- Binary applies Landlock/seccomp then exec's the command

**Landlock filesystem rules**:
```rust
// Allow read-only to entire filesystem
.add_rules(landlock::path_beneath_rules(&["/"], access_ro))
// Allow read-write to /dev/null
.add_rules(landlock::path_beneath_rules(&["/dev/null"], access_rw))
// Allow read-write to writable_roots
.add_rules(landlock::path_beneath_rules(&writable_roots, access_rw))
```

Uses ABI V5 with BestEffort compatibility.

**seccomp network filtering**:
Blocks network-related syscalls:
- connect, accept, accept4, bind, listen
- sendto, sendmsg, sendmmsg
- recvmsg, recvmmsg (but allows recvfrom for cargo clippy)
- getsockopt, setsockopt
- ptrace (also blocked)

Special handling:
- socket/socketpair: Only allows AF_UNIX domain sockets
- Returns EPERM when blocked syscall is attempted
- Default action: Allow (only blocks specific syscalls)

**Process flow**:
1. Parse command-line args (sandbox policy JSON, cwd, command)
2. Apply sandbox to current thread (child inherits)
3. exec() the target command

### UI/CLI Integration

**Files**: `codex-rs/cli/src/debug_sandbox.rs`

Users can test sandbox with:
```bash
codex sandbox macos [--full-auto] [COMMAND]...
codex sandbox linux [--full-auto] [COMMAND]...
```

**Sandbox selection** (`codex-rs/core/src/sandboxing/mod.rs`):
- Platform-specific: macOS uses Seatbelt, Linux uses Landlock+seccomp
- Automatic selection based on OS
- Falls back to no sandbox if platform support unavailable

**Approval policies** (from docs/sandbox.md):
- `untrusted` - Ask for everything except safe reads
- `on-failure` - Auto-approve sandboxed, ask on failure
- `on-request` - Model decides when to ask
- `never` - Never ask (still honors sandbox)

### Special Protections

1. **.git folder protection**: Read-only even in workspace-write mode
   - Prevents accidental corruption of git history
   - Implemented in both macOS and Linux

2. **Path canonicalization**: Resolves symlinks to prevent bypass
   - macOS: Handles /var → /private/var mapping
   - Linux: Canonical paths for Landlock rules

3. **Environment variables**:
   - `CODEX_SANDBOX_ENV_VAR=seatbelt` (macOS)
   - `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR=1` (when network disabled)

4. **Executable path hardening**:
   - macOS: Only uses `/usr/bin/sandbox-exec` (not PATH lookup)
   - Linux: Uses bundled `codex-linux-sandbox` binary

### Windows Implementation (Experimental)

**File**: `codex-rs/windows-sandbox-rs/`

Uses Windows-specific mechanisms:
- Restricted token with AppContainer profile
- Capability SIDs for filesystem access
- Environment variable overrides for network blocking
- Stub executables for network tools

**Limitations** (from docs):
- Cannot prevent writes to world-writable folders
- Highly experimental
- Recommendation: Use container-based isolation instead

## Investigation Complete

### Files Created
1. `notes.md` - This file, tracking investigation progress
2. `README.md` - Comprehensive report on sandbox implementation
3. `macos-seatbelt-snippet.txt` - Code snippet showing macOS implementation
4. `linux-landlock-snippet.txt` - Code snippet showing Linux implementation
5. `git-protection-snippet.txt` - Code snippet showing .git protection

### Key Learnings

1. **Platform-specific but unified API**: The sandbox uses different OS mechanisms but presents consistent behavior
2. **Defense in depth**: Multiple layers (filesystem + network + syscalls)
3. **Developer-friendly**: Balances security with usability (workspace-write mode)
4. **VCS-aware**: Special protection for .git folders is a unique feature
5. **Well-tested**: Extensive test suite and debug commands available

### Sources Examined
- `docs/sandbox.md` - High-level documentation
- `codex-rs/core/src/seatbelt.rs` - macOS implementation
- `codex-rs/core/src/landlock.rs` - Linux integration
- `codex-rs/linux-sandbox/src/landlock.rs` - Linux core implementation
- `codex-rs/linux-sandbox/src/linux_run_main.rs` - Linux binary entry point
- `codex-rs/protocol/src/protocol.rs` - Protocol definitions and WritableRoot
- `codex-rs/cli/src/debug_sandbox.rs` - CLI integration
- `seatbelt_base_policy.sbpl` - macOS Seatbelt base policy
- `seatbelt_network_policy.sbpl` - macOS Seatbelt network policy
