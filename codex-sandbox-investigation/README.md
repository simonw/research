# OpenAI Codex CLI Sandbox Implementation Analysis

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

## Executive Summary

This report provides a comprehensive analysis of the sandboxing implementation in OpenAI's Codex CLI (https://github.com/openai/codex). The sandbox is a critical security feature that restricts the Codex AI agent's ability to modify files, access the network, and execute potentially dangerous operations while still allowing it to perform useful development tasks.

## High-Level Overview: What the Sandbox Allows and Denies

### Sandbox Operating Modes

The Codex CLI provides three primary sandbox modes:

#### 1. DangerFullAccess (No Sandbox)
- **Allows**: Everything - complete filesystem access, network access, all operations
- **Denies**: Nothing
- **Use Case**: Only when explicitly requested by user with `--dangerously-bypass-approvals-and-sandbox` or `--yolo` flags
- **Warning**: Not recommended; bypasses all security protections

#### 2. ReadOnly Mode
- **Allows**:
  - Reading files from entire filesystem
  - Executing programs (but their effects are sandboxed)
  - Process forking and creation
  - Reading system information (sysctls, process info)
  - IPC mechanisms (POSIX semaphores)
- **Denies**:
  - Writing to any files (except `/dev/null`)
  - Network access (all network syscalls blocked)
  - Modifying filesystem state
- **Use Case**: Safe exploration of codebases; answering questions; non-interactive CI/CD
- **Default**: For untrusted workspaces

#### 3. WorkspaceWrite Mode
- **Allows**:
  - Everything from ReadOnly mode
  - Writing to current working directory (workspace)
  - Writing to `/tmp` and `$TMPDIR` (configurable)
  - Writing to additional specified `writable_roots`
  - Optionally: Network access (disabled by default)
- **Denies**:
  - Writing outside workspace/designated roots
  - Network access (unless explicitly enabled in config)
  - **Writing to `.git` folders (special protection)**
  - Writing to read-only subpaths within writable roots
- **Use Case**: Trusted repositories where agent needs to edit code
- **Default**: After user marks workspace as trusted

### Special Protections

#### Git Repository Protection
The sandbox implements special protection for version control metadata:
- `.git` folders are **always read-only**, even within writable workspaces
- Prevents accidental corruption of git history, staged changes, or repository state
- Implemented at the OS sandbox level on both macOS and Linux

#### Path Canonicalization
- All paths are canonicalized to prevent symlink-based bypass attacks
- Handles OS-specific quirks (e.g., `/var` → `/private/var` on macOS)
- Ensures consistent enforcement regardless of how paths are referenced

#### Network Isolation
When network is disabled (default for WorkspaceWrite):
- All network-related syscalls return `EPERM`
- Only AF_UNIX domain sockets permitted (for local IPC)
- Environment variable `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR=1` set
- Tools like `cargo clippy` can still function via local process communication

### User Interaction & Approval Policies

The sandbox works in conjunction with approval policies:

| Approval Policy | Behavior |
|----------------|----------|
| `untrusted` | Ask for everything except safe read-only commands |
| `on-failure` | Auto-approve sandboxed operations; ask only when sandbox blocks |
| `on-request` | Model decides when to request approval |
| `never` | Never ask user; sandbox still enforced |

Users can test sandbox behavior:
```bash
codex sandbox macos [--full-auto] [COMMAND]...
codex sandbox linux [--full-auto] [COMMAND]...
```

---

## Implementation Details

### Platform-Specific Architecture

Codex uses different sandboxing mechanisms depending on the operating system:

| Platform | Primary Mechanism | Secondary Mechanism | Availability |
|----------|------------------|---------------------|--------------|
| macOS 12+ | Apple Seatbelt | N/A | Stable |
| Linux | Landlock (filesystem) | seccomp (network/syscalls) | Requires kernel 5.13+ |
| Windows | AppContainer + Restricted Token | Environment variable blocking | Experimental |

---

## macOS Implementation: Apple Seatbelt

### Overview

On macOS, Codex uses Apple's `sandbox-exec` command-line utility, which interfaces with the kernel-level Seatbelt/Sandbox.kext mandatory access control framework.

**Key Implementation File**: `codex-rs/core/src/seatbelt.rs`

### Architecture

1. **Hardened Executable Path**: Only uses `/usr/bin/sandbox-exec` (not PATH lookup)
   - Prevents injection of malicious sandbox-exec alternatives
   - Assumes if this binary is compromised, attacker has root already

2. **Dynamic Policy Generation**: Builds Scheme-based policy files at runtime
   - Base policy: `seatbelt_base_policy.sbpl` (deny-by-default)
   - Network policy: `seatbelt_network_policy.sbpl` (when network enabled)
   - File write policies generated dynamically based on writable roots

3. **Parameterized Policies**: Uses `-D` flags to pass paths
   - Example: `-DWRITABLE_ROOT_0=/path/to/workspace`
   - Avoids shell injection; paths handled as data not code

### Base Policy (seatbelt_base_policy.sbpl)

**Philosophy**: Deny by default, explicitly allow what's needed

```scheme
(version 1)

; start with closed-by-default
(deny default)

; Allow child processes to inherit sandbox
(allow process-exec)
(allow process-fork)
(allow signal (target same-sandbox))

; Allow reading user preferences and process info
(allow user-preference-read)
(allow process-info* (target same-sandbox))

; Allow writing to /dev/null only
(allow file-write-data
  (require-all
    (path "/dev/null")
    (vnode-type CHARACTER-DEVICE)))
```

**Allowed sysctls** (read-only):
- Hardware info: `hw.activecpu`, `hw.cputype`, `hw.memsize`, `hw.ncpu`, etc.
- Kernel info: `kern.osversion`, `kern.osrelease`, `kern.hostname`
- VM info: `vm.loadavg`
- Performance levels: `hw.perflevel*`, `hw.optional.arm.*`

**Allowed IOKit**:
- `RootDomainUserClient` (power management)

**Allowed Mach lookups**:
- `com.apple.system.opendirectoryd.libinfo` (user/group lookup)
- `com.apple.PowerManagement.control`

**IPC**:
- POSIX semaphores allowed (needed for Python multiprocessing)

### Filesystem Access Control

The implementation builds Seatbelt policy rules dynamically:

**For full disk write** (DangerFullAccess):
```scheme
(allow file-write* (regex #"^/"))
```

**For workspace write** (with .git protection):
```scheme
; Generated for each writable root
(allow file-write*
  ; Workspace root writable, but exclude .git
  (require-all
    (subpath (param "WRITABLE_ROOT_0"))
    (require-not (subpath (param "WRITABLE_ROOT_0_RO_0")))
  )
  ; Additional writable roots
  (subpath (param "WRITABLE_ROOT_1"))
  (subpath (param "WRITABLE_ROOT_2"))
)
```

**Implementation** (from `create_seatbelt_command_args`):

```rust
for (index, wr) in writable_roots.iter().enumerate() {
    // Canonicalize to avoid mismatches like /var vs /private/var on macOS.
    let canonical_root = wr.root.canonicalize().unwrap_or_else(|_| wr.root.clone());
    let root_param = format!("WRITABLE_ROOT_{index}");
    file_write_params.push((root_param.clone(), canonical_root));

    if wr.read_only_subpaths.is_empty() {
        writable_folder_policies.push(format!("(subpath (param \"{root_param}\"))"));
    } else {
        // Build (require-all (subpath ROOT) (require-not (subpath .git)))
        let mut require_parts: Vec<String> = Vec::new();
        require_parts.push(format!("(subpath (param \"{root_param}\"))"));
        for (subpath_index, ro) in wr.read_only_subpaths.iter().enumerate() {
            let canonical_ro = ro.canonicalize().unwrap_or_else(|_| ro.clone());
            let ro_param = format!("WRITABLE_ROOT_{index}_RO_{subpath_index}");
            require_parts.push(format!("(require-not (subpath (param \"{ro_param}\")))"));
            file_write_params.push((ro_param, canonical_ro));
        }
        let policy_component = format!("(require-all {} )", require_parts.join(" "));
        writable_folder_policies.push(policy_component);
    }
}
```

### Network Policy

When network access is enabled, the following is appended to the policy:

```scheme
(allow network-outbound)
(allow network-inbound)
(allow system-socket)

(allow mach-lookup
    ; BSD directory helper for cache dirs
    (global-name "com.apple.bsd.dirhelper")
    (global-name "com.apple.system.opendirectoryd.membership")

    ; TLS/certificate services
    (global-name "com.apple.SecurityServer")
    (global-name "com.apple.networkd")
    (global-name "com.apple.ocspd")
    (global-name "com.apple.trustd.agent")

    ; Network configuration
    (global-name "com.apple.SystemConfiguration.DNSConfiguration")
    (global-name "com.apple.SystemConfiguration.configd")
)

(allow file-write*
  (subpath (param "DARWIN_USER_CACHE_DIR"))
)
```

### Command Execution Flow

1. Codex builds the policy string with parameters
2. Constructs command: `/usr/bin/sandbox-exec -p <policy> -DWRITABLE_ROOT_0=<path> ... -- <user_command>`
3. Spawns process with policy applied
4. All child processes inherit the sandbox restrictions
5. Environment variable set: `CODEX_SANDBOX_ENV_VAR=seatbelt`

**Example generated command**:
```bash
/usr/bin/sandbox-exec \
  -p '(version 1) (deny default) (allow file-read*) (allow file-write* (require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0")))))' \
  -DWRITABLE_ROOT_0=/Users/dev/project \
  -DWRITABLE_ROOT_0_RO_0=/Users/dev/project/.git \
  -- \
  npm install
```

---

## Linux Implementation: Landlock + seccomp

### Overview

On Linux, Codex uses a two-layer approach:
1. **Landlock** (Linux 5.13+) - Filesystem access control via LSM (Linux Security Module)
2. **seccomp** - System call filtering for network isolation

**Key Implementation Files**:
- `codex-rs/core/src/landlock.rs` - Integration layer
- `codex-rs/linux-sandbox/src/landlock.rs` - Core implementation
- `codex-rs/linux-sandbox/src/linux_run_main.rs` - Standalone binary entry point

### Architecture

Unlike macOS which uses an external `sandbox-exec` binary, Linux uses a **separate helper binary** (`codex-linux-sandbox`) that:

1. Receives serialized `SandboxPolicy` as JSON via CLI args
2. Applies Landlock filesystem restrictions
3. Applies seccomp syscall filters
4. Executes (`execvp`) the target command

This design ensures the sandbox is applied **before** the target command runs.

**Command structure**:
```bash
codex-linux-sandbox \
  --sandbox-policy-cwd /path/to/workspace \
  --sandbox-policy '{"mode":"workspace-write","writable_roots":[...],"network_access":false}' \
  -- \
  npm install
```

### Landlock Filesystem Restrictions

**Implementation** (`install_filesystem_landlock_rules_on_current_thread`):

```rust
fn install_filesystem_landlock_rules_on_current_thread(writable_roots: Vec<PathBuf>) -> Result<()> {
    let abi = ABI::V5;  // Uses latest Landlock ABI version
    let access_rw = AccessFs::from_all(abi);
    let access_ro = AccessFs::from_read(abi);

    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(access_rw)?
        .create()?
        // Global read-only access to entire filesystem
        .add_rules(landlock::path_beneath_rules(&["/"], access_ro))?
        // Allow writing to /dev/null
        .add_rules(landlock::path_beneath_rules(&["/dev/null"], access_rw))?
        .set_no_new_privs(true);

    // Add writable roots (workspace, /tmp, etc.)
    if !writable_roots.is_empty() {
        ruleset = ruleset.add_rules(landlock::path_beneath_rules(&writable_roots, access_rw))?;
    }

    // Apply to current thread (inherited by exec'd process)
    let status = ruleset.restrict_self()?;

    if status.ruleset == landlock::RulesetStatus::NotEnforced {
        return Err(CodexErr::Sandbox(SandboxErr::LandlockRestrict));
    }

    Ok(())
}
```

**Key aspects**:

1. **Hierarchical rules**: Later rules can refine earlier ones
2. **Read-only root**: `path_beneath_rules(&["/"], access_ro)` grants read access to entire filesystem
3. **Write allowances**: Specific paths get read-write access
4. **No new privileges**: Prevents privilege escalation via setuid/setgid
5. **Best effort compatibility**: Works across different kernel versions supporting Landlock

**Landlock ABI V5 permissions**:
- Read: `execute`, `read_file`, `read_dir`
- Write: `write_file`, `remove_dir`, `remove_file`, `make_char`, `make_dir`, `make_reg`, `make_sock`, `make_fifo`, `make_block`, `make_sym`, `truncate`, `refer`

### seccomp Network Filtering

While Landlock handles filesystem access, **seccomp-bpf** (Berkeley Packet Filter) blocks network-related syscalls.

**Implementation** (`install_network_seccomp_filter_on_current_thread`):

```rust
fn install_network_seccomp_filter_on_current_thread() -> std::result::Result<(), SandboxErr> {
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    // Unconditionally deny these syscalls
    let mut deny_syscall = |nr: i64| {
        rules.insert(nr, vec![]); // empty vec = always match, return EPERM
    };

    // Network syscalls blocked
    deny_syscall(libc::SYS_connect);
    deny_syscall(libc::SYS_accept);
    deny_syscall(libc::SYS_accept4);
    deny_syscall(libc::SYS_bind);
    deny_syscall(libc::SYS_listen);
    deny_syscall(libc::SYS_getpeername);
    deny_syscall(libc::SYS_getsockname);
    deny_syscall(libc::SYS_shutdown);
    deny_syscall(libc::SYS_sendto);
    deny_syscall(libc::SYS_sendmsg);
    deny_syscall(libc::SYS_sendmmsg);
    deny_syscall(libc::SYS_recvmsg);
    deny_syscall(libc::SYS_recvmmsg);
    deny_syscall(libc::SYS_getsockopt);
    deny_syscall(libc::SYS_setsockopt);
    deny_syscall(libc::SYS_ptrace);  // Also blocked for security

    // Special handling for socket(): only allow AF_UNIX
    let unix_only_rule = SeccompRule::new(vec![SeccompCondition::new(
        0,                          // arg0 (domain)
        SeccompCmpArgLen::Dword,
        SeccompCmpOp::Ne,           // Not equal
        libc::AF_UNIX as u64,       // Deny if not AF_UNIX
    )?])?;

    rules.insert(libc::SYS_socket, vec![unix_only_rule]);

    // Build BPF program
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,                     // Default: allow syscalls
        SeccompAction::Errno(libc::EPERM as u32), // On match: return EPERM
        TargetArch::x86_64,  // or aarch64
    )?;

    let prog: BpfProgram = filter.try_into()?;
    apply_filter(&prog)?;
    Ok(())
}
```

**Notable decisions**:

1. **recvfrom allowed**: Not blocked to support tools like `cargo clippy` that use socketpairs for IPC
2. **AF_UNIX allowed**: Local sockets for process communication still work
3. **ptrace blocked**: Prevents debugging/inspection of other processes
4. **EPERM error**: Network attempts fail with "Operation not permitted"

### Process Execution Flow

1. Main Codex process calls `codex-linux-sandbox` binary
2. Helper parses `--sandbox-policy` JSON and `--sandbox-policy-cwd`
3. Helper calls `apply_sandbox_policy_to_current_thread()`:
   - Applies network seccomp filter if `network_access == false`
   - Applies Landlock filesystem rules if not `DangerFullAccess`
4. Helper calls `execvp()` to replace itself with target command
5. Target command inherits all sandbox restrictions

**Entry point** (`linux_run_main.rs`):

```rust
pub fn run_main() -> ! {
    let LandlockCommand {
        sandbox_policy_cwd,
        sandbox_policy,
        command,
    } = LandlockCommand::parse();

    if let Err(e) = apply_sandbox_policy_to_current_thread(&sandbox_policy, &sandbox_policy_cwd) {
        panic!("error running landlock: {e:?}");
    }

    let c_command = CString::new(command[0].as_str()).expect("...");
    let c_args: Vec<CString> = command.iter().map(|arg| CString::new(arg.as_str()).expect("...")).collect();
    let mut c_args_ptrs: Vec<*const libc::c_char> = c_args.iter().map(|arg| arg.as_ptr()).collect();
    c_args_ptrs.push(std::ptr::null());

    unsafe {
        libc::execvp(c_command.as_ptr(), c_args_ptrs.as_ptr());
    }

    // If we reach here, execvp failed
    let err = std::io::Error::last_os_error();
    panic!("Failed to execvp {}: {err}", command[0].as_str());
}
```

### Kernel Requirements

- **Landlock**: Linux kernel 5.13+ required
- **seccomp**: Linux kernel 3.5+ (widely available)
- **Best effort**: Degrades gracefully on older kernels

If Landlock is unavailable, Codex may fall back to no sandboxing (with warning) or require containerization.

---

## Cross-Platform: Git Repository Protection

Both macOS and Linux implement identical `.git` folder protection at a higher abstraction level.

### Mechanism

**Data structure** (`codex-rs/protocol/src/protocol.rs`):

```rust
/// A writable root path accompanied by a list of subpaths that should remain
/// read‑only even when the root is writable. This is primarily used to ensure
/// top‑level VCS metadata directories (e.g. `.git`) under a writable root are
/// not modified by the agent.
pub struct WritableRoot {
    pub root: PathBuf,
    pub read_only_subpaths: Vec<PathBuf>,
}

impl WritableRoot {
    pub fn is_path_writable(&self, path: &Path) -> bool {
        // Check if the path is under the root.
        if !path.starts_with(&self.root) {
            return false;
        }

        // Check if the path is under any of the read-only subpaths.
        for subpath in &self.read_only_subpaths {
            if path.starts_with(subpath) {
                return false;
            }
        }

        true
    }
}
```

**Automatic detection**:

When building the list of writable roots, Codex automatically detects top-level `.git` directories:

```rust
roots
    .into_iter()
    .map(|writable_root| {
        let mut subpaths = Vec::new();
        let top_level_git = writable_root.join(".git");
        if top_level_git.is_dir() {
            subpaths.push(top_level_git);
        }
        WritableRoot {
            root: writable_root,
            read_only_subpaths: subpaths,
        }
    })
    .collect()
```

### Platform Translation

**macOS (Seatbelt)**:
- Generates `(require-not (subpath "/path/to/workspace/.git"))` policy
- OS enforces at syscall level

**Linux (Landlock)**:
- `.git` **not included** in `writable_roots` passed to Landlock
- Only parent workspace gets write access
- Landlock enforces via path hierarchy

### Security Implications

1. **Prevents accidental damage**: Agent can't corrupt repository state
2. **Protects history**: Commits, refs, objects remain untouched
3. **Staged changes safe**: Index file in `.git/index` is read-only
4. **Git operations still work**: Agent can still call `git` CLI tools (they operate outside sandbox or request approval)

---

## Windows Implementation (Experimental)

**Status**: Highly experimental; not recommended for production use

**File**: `codex-rs/windows-sandbox-rs/`

### Approach

Windows lacks direct equivalents to Seatbelt or Landlock, so Codex uses:

1. **AppContainer + Restricted Token**
   - Creates a restricted security token
   - Attaches AppContainer profile with capability SIDs
   - Grants filesystem access via specific capabilities

2. **Environment Variable Blocking**
   - Overrides `HTTP_PROXY`, `HTTPS_PROXY`, etc.
   - Inserts stub executables for network tools

### Limitations

Per documentation:
> "It cannot prevent file writes, deletions, or creations in any directory where the Everyone SID already has write permissions (for example, world-writable folders)."

**Recommendation**: Use WSL2 or Docker for proper isolation on Windows.

---

## Security Analysis

### Strengths

1. **Defense in Depth**:
   - OS-level enforcement (not just process-level)
   - Multiple layers (filesystem + network + syscalls)
   - Deny-by-default policies

2. **Path Hardening**:
   - Canonicalization prevents symlink attacks
   - Absolute paths avoid relative path confusion
   - Special handling for OS quirks

3. **VCS Protection**:
   - Automatic `.git` detection and protection
   - Prevents corruption of repository state

4. **Least Privilege**:
   - Minimal sysctls/services exposed
   - Only AF_UNIX sockets when network disabled
   - No new privileges flag on Linux

5. **Transparency**:
   - Test commands available (`codex sandbox`)
   - Clear documentation
   - Open source for auditing

### Potential Limitations

1. **Kernel Dependencies**:
   - Linux requires 5.13+ for Landlock
   - Older systems may lack sandboxing

2. **Container Compatibility**:
   - Docker/Podman may not expose Landlock/seccomp
   - Recommendation: Use container-level isolation

3. **Windows Support**:
   - Experimental; significant limitations
   - Better to use WSL2/containerization

4. **Approval Policy Complexity**:
   - Users may not understand policy interactions
   - `--yolo` flag bypasses all protections (risky)

5. **Read Access**:
   - Full filesystem read access in all modes
   - Could leak sensitive files (credentials, keys, etc.)
   - Relies on user awareness

### Attack Vectors Mitigated

| Attack | Mitigation |
|--------|-----------|
| Path traversal to write outside workspace | Landlock/Seatbelt path rules |
| Symlink attack to bypass restrictions | Path canonicalization |
| Network data exfiltration | seccomp/Seatbelt blocks network syscalls |
| Git repository corruption | `.git` read-only protection |
| Privilege escalation via setuid | `set_no_new_privs` on Linux |
| Injecting malicious sandbox-exec | Hardcoded `/usr/bin/sandbox-exec` path |
| Shell injection in policy | Parameterized policies with `-D` flags |

---

## Comparison with Similar Systems

| Feature | Codex | Docker | Firejail | Chrome Sandbox |
|---------|-------|--------|----------|----------------|
| macOS Seatbelt | ✅ | ❌ | ❌ | ✅ |
| Linux Landlock | ✅ | ❌ | ❌ | ❌ |
| Linux seccomp | ✅ | ✅ | ✅ | ✅ |
| User namespaces | ❌ | ✅ | ✅ | ✅ |
| Network namespaces | ❌ | ✅ | ✅ | ✅ |
| Git protection | ✅ | ❌ | ❌ | N/A |
| Dynamic policies | ✅ | ❌ | ⚠️ | ⚠️ |
| In-process application | ✅ | ❌ | ❌ | ✅ |

**Key Differentiator**: Codex focuses on **workspace-aware** sandboxing with VCS protection, rather than full isolation. This allows better integration with development workflows while maintaining security.

---

## Configuration Examples

### config.toml Examples

**Read-only mode** (untrusted workspace):
```toml
approval_policy = "untrusted"
sandbox_mode = "read-only"
```

**Full-auto mode** (trusted workspace):
```toml
approval_policy = "on-request"
sandbox_mode = "workspace-write"
```

**Workspace write with network**:
```toml
approval_policy = "on-request"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
network_access = true
```

**Custom profiles**:
```toml
[profiles.safe_testing]
approval_policy = "never"
sandbox_mode = "workspace-write"

[profiles.readonly_quiet]
approval_policy = "never"
sandbox_mode = "read-only"
```

### CLI Usage

**Test sandbox**:
```bash
# macOS
codex sandbox macos --full-auto -- npm install

# Linux
codex sandbox linux -- cargo build
```

**Run with specific mode**:
```bash
codex --sandbox read-only --ask-for-approval on-request
codex --sandbox workspace-write --ask-for-approval never
codex --full-auto  # equivalent to workspace-write + on-request
```

---

## Conclusion

The OpenAI Codex CLI implements a sophisticated, platform-specific sandboxing system that balances security with usability for AI-assisted software development:

- **macOS**: Leverages Apple's mature Seatbelt framework with dynamic Scheme policy generation
- **Linux**: Combines modern Landlock LSM with battle-tested seccomp syscall filtering
- **Cross-platform**: Consistent high-level API with platform-appropriate enforcement

The sandbox provides:
1. **Filesystem isolation** - Read-only or workspace-scoped write access
2. **Network isolation** - Syscall-level blocking with AF_UNIX exceptions
3. **VCS protection** - Automatic `.git` folder read-only enforcement
4. **Security hardening** - Path canonicalization, executable hardening, privilege restrictions

This implementation demonstrates best practices for sandboxing untrusted code execution in development tools, with particular attention to the unique requirements of AI code agents.

---

## References

- OpenAI Codex Repository: https://github.com/openai/codex
- Documentation: https://github.com/openai/codex/blob/main/docs/sandbox.md
- Apple Sandbox Guide: https://developer.apple.com/library/archive/documentation/Security/Conceptual/AppSandboxDesignGuide/
- Linux Landlock: https://docs.kernel.org/userspace-api/landlock.html
- Linux seccomp: https://www.kernel.org/doc/html/latest/userspace-api/seccomp_filter.html
- Chrome Sandbox: https://chromium.googlesource.com/chromium/src/+/main/docs/design/sandbox.md

---

**Investigation Date**: November 2025
**Repository Commit**: Latest from main branch
**Codex Version**: Analyzed from current main branch
