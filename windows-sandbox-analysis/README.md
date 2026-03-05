# Windows Sandbox Implementation in OpenAI Codex (`codex-rs/windows-sandbox-rs`)

A detailed analysis of the Windows sandboxing mechanisms used by [OpenAI Codex](https://github.com/openai/codex) to isolate AI-generated code execution on Windows.

## Table of Contents

1. [Overview](#overview)
2. [Sandbox Capabilities](#sandbox-capabilities)
3. [Architecture](#architecture)
4. [Implementation Details](#implementation-details)
   - [Sandbox User Provisioning](#1-sandbox-user-provisioning)
   - [Restricted Token Creation](#2-restricted-token-creation)
   - [ACL-Based Filesystem Control](#3-acl-based-filesystem-control)
   - [Network Isolation](#4-network-isolation)
   - [Environment Poisoning](#5-environment-poisoning)
   - [Process Lifecycle Management](#6-process-lifecycle-management)
   - [Credential Management](#7-credential-management)
   - [World-Writable Audit](#8-world-writable-directory-audit)
   - [Workspace Protection](#9-workspace-protection)
   - [User Hiding](#10-user-hiding)
   - [CWD Junction Workaround](#11-cwd-junction-workaround)
5. [Security Model Summary](#security-model-summary)

---

## Overview

The `codex-rs/windows-sandbox-rs` crate implements a multi-layered sandboxing system for executing AI-generated commands on Windows. Rather than using Windows Sandbox (WSB) or Hyper-V containers, it builds isolation from native Windows security primitives: **dedicated local user accounts**, **restricted tokens** (`CreateRestrictedToken`), **NTFS DACLs**, **Windows Firewall rules**, and **environment variable manipulation**.

The sandbox supports two policy modes:
- **ReadOnly** - The sandboxed process can read the filesystem but cannot write anywhere except designated temp paths
- **WorkspaceWrite** - The sandboxed process can read the filesystem and write to a designated workspace directory (and its temp directories), but not to protected subdirectories like `.git`, `.codex`, or `.agents`

Two additional policy values (`DangerFullAccess` and `ExternalSandbox`) are explicitly rejected by the Windows sandbox backend.

## Sandbox Capabilities

### What the sandbox prevents

| Threat | Mechanism |
|--------|-----------|
| **Writing to arbitrary filesystem locations** | Restricted token with `WRITE_RESTRICTED` flag; ACL deny ACEs on non-writable paths |
| **Writing to `.git`, `.codex`, `.agents` directories** | Explicit deny-write ACEs applied to these protected subdirectories |
| **Network access (when policy disallows)** | Windows Firewall outbound block rule scoped to the offline sandbox user's SID |
| **Network access via common tools** | Proxy environment variables pointed at dead endpoints; stub scripts for `ssh`/`scp` |
| **Privilege escalation** | Token created with `DISABLE_MAX_PRIVILEGE` and `LUA_TOKEN` flags; process runs under a standard-privilege user account |
| **Accessing secrets/credentials** | Sandbox secrets directory has explicit deny ACE for the sandbox group; USERPROFILE exclusions for `.ssh`, `.gnupg`, `.aws`, etc. |
| **Tampering with sandbox infrastructure** | Sandbox directories (`.sandbox`, `.sandbox-bin`, `.sandbox-secrets`) have locked-down DACLs |
| **Cross-workspace contamination** | Per-workspace capability SIDs ensure one workspace's sandbox cannot write to another's files |
| **Modifying world-writable directories** | Pre-execution audit scans for `Everyone`-writable directories and applies deny ACEs |

### What the sandbox allows

| Capability | Details |
|------------|---------|
| **Full disk read** | Both ReadOnly and WorkspaceWrite policies grant full disk read access (restricted read-only is not yet implemented) |
| **Workspace writes (WorkspaceWrite mode)** | Write access to the CWD and any additional `writable_roots` specified in the policy |
| **Temp directory writes** | `TEMP`/`TMP` environment variable paths are included as writable (unless `exclude_tmpdir_env_var` is set) |
| **Process execution** | The sandboxed process can spawn child processes, but they inherit the restricted token |
| **NUL device access** | Explicit ACE grants on `\\.\NUL` so stdout/stderr redirection works |

## Architecture

The sandbox has three main executables and a library:

```
codex (main process)
  │
  ├── codex-windows-sandbox (library)
  │     Orchestrates setup, token creation, ACL management
  │
  ├── codex-windows-sandbox-setup.exe (elevated helper)
  │     Runs with admin privileges (UAC) to:
  │       - Create sandbox user accounts
  │       - Configure firewall rules
  │       - Apply filesystem ACLs
  │
  └── codex-command-runner.exe (sandbox runner)
        Runs as the sandbox user to:
          - Create a restricted token
          - Spawn the actual command under that token
          - Bridge stdio via named pipes
```

### Execution Flow

```
1. Main Codex process receives a command to execute
2. Parse sandbox policy (ReadOnly or WorkspaceWrite)
3. Check if sandbox setup is complete (marker + users files exist)
4. If not, launch elevated setup via ShellExecuteExW("runas") → UAC prompt
   a. Setup creates sandbox user accounts (CodexSandboxOffline, CodexSandboxOnline)
   b. Setup configures firewall rules
   c. Setup applies read/write ACLs to filesystem roots
   d. Setup writes DPAPI-encrypted credentials
5. Refresh ACLs for the current workspace (non-elevated)
6. Load sandbox user credentials (DPAPI decrypt)
7. Choose offline or online user based on network policy
8. Create named pipes for stdin/stdout/stderr
9. Launch codex-command-runner.exe as sandbox user via CreateProcessWithLogonW
10. Command runner:
    a. Creates a restricted token from its own process token
    b. Opens named pipes for stdio
    c. Spawns actual command via CreateProcessAsUserW with restricted token
    d. Assigns child to a Job Object with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
    e. Waits for completion, relays exit code
11. Main process reads stdout/stderr from pipes, returns result
```

## Implementation Details

### 1. Sandbox User Provisioning

**File:** `sandbox_users.rs`, `setup_main_win.rs`

The sandbox creates two dedicated Windows local user accounts:

- **`CodexSandboxOffline`** - Used when the policy does not allow network access
- **`CodexSandboxOnline`** - Used when the policy allows network access

Both accounts are created via `NetUserAdd` with `USER_PRIV_USER` privilege level and `UF_DONT_EXPIRE_PASSWD` flag. They are added to a custom local group called **`CodexSandboxUsers`** (created via `NetLocalGroupAdd`) and to the standard `Users` group.

The two-user design enables network isolation: the firewall rule targets only the offline user's SID, so the online user retains network access while the offline user is completely blocked.

**Password management:** Each user gets a random 24-character password generated from a character set of uppercase, lowercase, digits, and symbols. Passwords are encrypted using DPAPI (`CryptProtectData` with `CRYPTPROTECT_LOCAL_MACHINE` scope) and stored base64-encoded in `CODEX_HOME/.sandbox-secrets/sandbox_users.json`.

### 2. Restricted Token Creation

**File:** `token.rs`

This is the core privilege-restriction mechanism. The sandbox uses the Windows `CreateRestrictedToken` API with three critical flags:

```rust
const DISABLE_MAX_PRIVILEGE: u32 = 0x01;
const LUA_TOKEN: u32 = 0x04;
const WRITE_RESTRICTED: u32 = 0x08;
```

- **`DISABLE_MAX_PRIVILEGE` (0x01):** Removes all privileges from the token except `SeChangeNotifyPrivilege` (which is re-enabled explicitly). This prevents the sandboxed process from performing privileged operations.

- **`LUA_TOKEN` (0x04):** Creates a Limited User Account token, stripping administrative group memberships. Even if the sandbox user were somehow added to an admin group, the token would behave as a standard user.

- **`WRITE_RESTRICTED` (0x08):** This is the key filesystem isolation mechanism. When a token has the `WRITE_RESTRICTED` flag, Windows performs a **second access check** for any write operation. The write is allowed only if the object's DACL contains an explicit allow ACE for one of the token's **restricting SIDs**. The restricting SIDs added to the token are:

  1. **Capability SIDs** (one or more randomly generated SIDs like `S-1-5-21-<random>-<random>-<random>-<random>`)
  2. **Logon SID** (the unique SID assigned to the current logon session)
  3. **Everyone SID** (`S-1-1-0`)

The **capability SIDs** are the mechanism that makes write control work. For a file to be writable by the sandboxed process, its DACL must contain an allow-write ACE for the specific capability SID. The sandbox grants these ACEs only on designated writable paths.

**Default DACL:** The restricted token's default DACL is set to grant `GENERIC_ALL` to the logon SID, Everyone SID, and all capability SIDs. This ensures that objects created by the sandboxed process (like pipes, temp files) are accessible to the process itself.

**Per-workspace capability SIDs:** In WorkspaceWrite mode, two types of capability SIDs are used:
- A **generic workspace SID** (`caps.workspace`) for general writable paths like temp directories
- A **per-CWD workspace SID** generated from a hash of the canonicalized CWD path, stored in `caps.workspace_by_cwd`

This means that a workspace at `C:\Projects\Alpha` gets a different capability SID than `C:\Projects\Beta`, preventing cross-workspace writes even if both use WorkspaceWrite mode.

### 3. ACL-Based Filesystem Control

**Files:** `acl.rs`, `allow.rs`, `workspace_acl.rs`, `setup_main_win.rs`

The filesystem access control works through Windows NTFS DACLs (Discretionary Access Control Lists). The sandbox manipulates DACLs using:

- `GetNamedSecurityInfoW` / `SetNamedSecurityInfoW` - Read/write DACLs by path
- `GetSecurityInfo` / `SetSecurityInfo` - Read/write DACLs by handle
- `SetEntriesInAclW` - Merge new ACEs into existing DACLs
- `EXPLICIT_ACCESS_W` structures with `TRUSTEE_IS_SID` trustee form

#### Allow ACEs

For writable paths (workspace CWD, temp directories, additional writable roots):
- Access mask: `FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE`
- Inheritance: `CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE` (applies to all children)
- Applied to: The appropriate capability SID

For read-only paths (Windows, Program Files, user profile, etc.):
- Access mask: `FILE_GENERIC_READ | FILE_GENERIC_EXECUTE`
- Applied to: The `CodexSandboxUsers` group SID (only if not already accessible via Users/Everyone/Authenticated Users)

#### Deny ACEs

Deny ACEs take precedence over allow ACEs in Windows and are used to block writes to sensitive locations:

- **Protected subdirectories** (`.git`, `.codex`, `.agents` inside writable roots):
  - Deny mask: `FILE_GENERIC_WRITE | FILE_WRITE_DATA | FILE_APPEND_DATA | FILE_WRITE_EA | FILE_WRITE_ATTRIBUTES | GENERIC_WRITE_MASK | DELETE | FILE_DELETE_CHILD`
  - Applied to: The capability SID

- **World-writable directories** (detected by audit scan):
  - Same deny mask, applied to prevent the sandbox from exploiting pre-existing permissive ACLs

#### NUL Device Access

The Windows NUL device (`\\.\NUL`) requires explicit ACE grants for the restricted token to use it for stdout/stderr redirection. The `allow_null_device` function opens `\\.\NUL` with `READ_CONTROL | WRITE_DAC` access and adds an allow ACE for the capability SID with `FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE`.

#### ACE Lifecycle

In ReadOnly mode, ACEs are **temporary** - they are added before command execution and revoked afterward using `REVOKE_ACCESS` mode. In WorkspaceWrite mode, ACEs are **persisted** to avoid churn across multiple command executions.

### 4. Network Isolation

**Files:** `firewall.rs`, `env.rs`

Network isolation uses two complementary mechanisms:

#### Windows Firewall Rules

The elevated setup helper creates a Windows Firewall rule using the COM API (`INetFwPolicy2`, `INetFwRule3`):

- **Rule name:** `codex_sandbox_offline_block_outbound`
- **Direction:** `NET_FW_RULE_DIR_OUT` (outbound only)
- **Action:** `NET_FW_ACTION_BLOCK`
- **Protocol:** `NET_FW_IP_PROTOCOL_ANY` (all protocols)
- **Profiles:** `NET_FW_PROFILE2_ALL` (all network profiles)
- **Scoped to:** The offline sandbox user's SID via `SetLocalUserAuthorizedList` with SDDL `O:LSD:(A;;CC;;;{offline_sid})`

This means only the `CodexSandboxOffline` user is affected. The `CodexSandboxOnline` user retains full network access. The rule is verified after creation by reading back the `LocalUserAuthorizedList` property and confirming it contains the expected SID.

#### Environment Variable Poisoning

Even with firewall rules, the sandbox adds defense-in-depth via environment variables:

```
HTTP_PROXY=http://127.0.0.1:9       # Dead proxy for HTTP
HTTPS_PROXY=http://127.0.0.1:9      # Dead proxy for HTTPS
ALL_PROXY=http://127.0.0.1:9        # Catch-all dead proxy
NO_PROXY=localhost,127.0.0.1,::1    # Only localhost bypasses proxy
GIT_HTTP_PROXY=http://127.0.0.1:9   # Git-specific dead proxy
GIT_HTTPS_PROXY=http://127.0.0.1:9
GIT_SSH_COMMAND=cmd /c exit 1        # SSH commands fail immediately
GIT_ALLOW_PROTOCOLS=                 # Block all git protocols
PIP_NO_INDEX=1                       # Prevent pip from fetching packages
NPM_CONFIG_OFFLINE=true              # npm offline mode
CARGO_NET_OFFLINE=true               # cargo offline mode
SBX_NONET_ACTIVE=1                   # Signal to tools that network is blocked
```

Additionally, stub `.bat`/`.cmd` scripts are created for `ssh` and `scp` in a `~/.sbx-denybin` directory (which is prepended to `PATH`). These stubs simply `exit /b 1`. The `PATHEXT` order is also adjusted to prioritize `.BAT` and `.CMD` over `.EXE`, ensuring the stubs are found before any real `ssh.exe`/`scp.exe`.

### 5. Environment Poisoning

**File:** `env.rs`

Beyond network blocking, the environment is manipulated for safety:

- **Null device normalization:** `/dev/null` references in environment variables are rewritten to `NUL` (the Windows equivalent)
- **Pager override:** `GIT_PAGER` and `PAGER` are set to `more.com` to prevent interactive pager invocations that could hang
- **PATH/PATHEXT inheritance:** The parent process's `PATH` and `PATHEXT` are inherited to ensure tools are discoverable
- **Git safe.directory:** When the CWD is inside a git repository, `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_N`/`GIT_CONFIG_VALUE_N` environment variables are set to add the repo root as a `safe.directory`, since the sandbox user doesn't own the repository

### 6. Process Lifecycle Management

**Files:** `elevated_impl.rs`, `command_runner_win.rs`, `process.rs`

The sandbox has a two-stage process launch:

#### Stage 1: Launch command runner as sandbox user

The main Codex process launches `codex-command-runner.exe` as the sandbox user using `CreateProcessWithLogonW`:

```rust
CreateProcessWithLogonW(
    user_w.as_ptr(),           // Sandbox username
    domain_w.as_ptr(),         // "." (local machine)
    password_w.as_ptr(),       // DPAPI-decrypted password
    LOGON_WITH_PROFILE,        // Load user profile
    exe_w.as_ptr(),            // Command runner exe path
    cmdline_vec.as_mut_ptr(),  // Command line
    CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
    env_block,                 // Inherited environment (no custom env block)
    cwd_w.as_ptr(),            // Working directory
    &si,                       // Startup info
    &mut pi,                   // Process info (output)
)
```

Communication happens via **named pipes** with permissive ACLs (`D:(A;;GA;;;WD)` - grant `GENERIC_ALL` to Everyone) so the sandbox user can connect:

```rust
let stdin_name = pipe_name("stdin");   // \\.\pipe\codex-runner-{random}-stdin
let stdout_name = pipe_name("stdout"); // \\.\pipe\codex-runner-{random}-stdout
let stderr_name = pipe_name("stderr"); // \\.\pipe\codex-runner-{random}-stderr
```

The request payload (policy, command, environment, pipe names, capability SIDs) is written to a temp file that the runner reads and immediately deletes.

#### Stage 2: Command runner creates restricted token and spawns command

The command runner (`codex-command-runner.exe`), now running as the sandbox user:

1. Parses the request payload from the file
2. Hides its own user profile directory (sets `HIDDEN | SYSTEM` attributes)
3. Creates a restricted token from its own process token using `CreateRestrictedToken`
4. Opens the named pipes for stdio
5. Optionally creates a CWD junction if the read-ACL helper is still running
6. Spawns the actual command using `CreateProcessAsUserW` with the restricted token
7. Assigns the child process to a **Job Object** with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` - this ensures all child processes are terminated when the runner exits
8. Waits for completion with optional timeout
9. Exits with the child's exit code

The desktop is explicitly set to `Winsta0\Default` to avoid `STATUS_DLL_INIT_FAILED` errors when launching processes like PowerShell with restricted tokens.

### 7. Credential Management

**Files:** `dpapi.rs`, `identity.rs`, `sandbox_users.rs`

Sandbox user passwords are stored encrypted using Windows DPAPI:

- **Encryption:** `CryptProtectData` with `CRYPTPROTECT_UI_FORBIDDEN | CRYPTPROTECT_LOCAL_MACHINE` flags
- **Decryption:** `CryptUnprotectData` with the same flags
- **Machine scope:** `CRYPTPROTECT_LOCAL_MACHINE` ensures any process on the machine can decrypt, regardless of which user runs it. This is necessary because the setup runs elevated (as admin) but the main Codex process runs as a regular user.

The credentials file (`sandbox_users.json`) lives in `CODEX_HOME/.sandbox-secrets/` with a locked-down DACL that explicitly **denies** the `CodexSandboxUsers` group all access, preventing the sandbox user from reading its own credentials.

Identity selection logic:
- If the policy has `network_access: false` → use `CodexSandboxOffline` user
- If the policy has `network_access: true` → use `CodexSandboxOnline` user

### 8. World-Writable Directory Audit

**File:** `audit.rs`

Before execution, the sandbox scans for directories that are writable by `Everyone` (`S-1-1-0`). This is a defense against path-injection attacks where a world-writable directory in `PATH` or `CWD` could be exploited.

The scan:
1. Checks CWD immediate children first (most likely attack surface)
2. Checks `TEMP`/`TMP` directories
3. Checks `USERPROFILE`, `PUBLIC`
4. Checks all `PATH` entries
5. Checks `C:\` and `C:\Windows` roots

For each directory, it queries the DACL and checks for any allow ACE granting `FILE_WRITE_DATA | FILE_APPEND_DATA | FILE_WRITE_EA | FILE_WRITE_ATTRIBUTES` to the `Everyone` SID.

The scan has safety limits:
- **Max 1000 items per directory** scanned
- **2-second time limit** for the entire scan
- **50,000 total items** checked
- Skips `Windows\Installer`, `Windows\Registration`, `ProgramData` subdirectories
- Skips symlinks/reparse points

Any flagged directories receive deny-write ACEs for the active capability SID, preventing the sandbox from writing to them even though they're world-writable.

### 9. Workspace Protection

**File:** `workspace_acl.rs`, `setup_main_win.rs`

Certain directories within the workspace receive special protection via deny-write ACEs:

- **`.git`** - Prevents the sandbox from modifying git history
- **`.codex`** - Prevents the sandbox from modifying Codex configuration
- **`.agents`** - Prevents the sandbox from modifying agent definitions

These deny ACEs are applied using the **per-workspace capability SID**, so they only affect the specific workspace's sandbox operations. The deny ACEs use the comprehensive write-block mask:

```
FILE_GENERIC_WRITE | FILE_WRITE_DATA | FILE_APPEND_DATA |
FILE_WRITE_EA | FILE_WRITE_ATTRIBUTES | GENERIC_WRITE_MASK |
DELETE | FILE_DELETE_CHILD
```

Additionally, the sandbox infrastructure directories have locked-down DACLs set by `lock_sandbox_dir`:

| Directory | Sandbox Group Access | Real User Access |
|-----------|---------------------|------------------|
| `.sandbox` | Read + Write + Execute + Delete | Read + Write + Execute |
| `.sandbox-bin` | Read + Execute only | Read + Write + Execute + Delete |
| `.sandbox-secrets` | **Denied** all access | Read + Write + Execute |

### 10. User Hiding

**File:** `hide_users.rs`

The sandbox users (`CodexSandboxOffline`, `CodexSandboxOnline`) are hidden from the Windows login screen via two mechanisms:

1. **Registry-based hiding:** Sets `DWORD` value `0` for each username under:
   ```
   HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList
   ```
   This prevents the users from appearing on the Windows login screen.

2. **Profile directory hiding:** When the command runner runs as a sandbox user and the profile directory exists, it sets `FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM` attributes on the profile directory to hide it from normal directory listings.

### 11. CWD Junction Workaround

**File:** `cwd_junction.rs`

A timing issue exists: the read-ACL helper runs asynchronously to grant read access to filesystem roots, but the command runner may start before those ACLs are applied. If the CWD's parent directories aren't yet readable by the sandbox user, `CreateProcessAsUserW` will fail.

The workaround creates an NTFS junction (directory symbolic link) under the sandbox user's profile:

```
%USERPROFILE%\.codex\.sandbox\cwd\{hash_of_cwd} → {actual_cwd}
```

The junction is created via `cmd /c mklink /J`, and is reused across invocations if it already exists. The command runner checks for the read-ACL mutex (`Local\CodexSandboxReadAcl`) to determine if the helper is still running and the junction is needed.

## Security Model Summary

The sandbox implements **defense in depth** with six independent layers:

```
Layer 1: User Isolation
  └─ Separate Windows user account (CodexSandboxOffline/Online)
     Process runs in a different security context

Layer 2: Token Restriction
  └─ CreateRestrictedToken with DISABLE_MAX_PRIVILEGE + LUA_TOKEN + WRITE_RESTRICTED
     All privileges stripped; writes require explicit capability SID ACE

Layer 3: Filesystem ACLs
  └─ Allow ACEs only on designated writable paths
     Deny ACEs on protected directories (.git, .codex, .agents)
     World-writable directory audit with deny ACEs

Layer 4: Network Isolation
  └─ Windows Firewall outbound block (offline user only)
     Targets all IP protocols across all network profiles

Layer 5: Environment Hardening
  └─ Dead proxy variables, tool stubs, protocol blocks
     Defense-in-depth even if firewall is bypassed

Layer 6: Infrastructure Protection
  └─ Locked DACLs on sandbox directories
     DPAPI-encrypted credentials inaccessible to sandbox user
     Per-workspace capability SIDs prevent cross-workspace contamination
```

Each layer provides independent protection. Even if one layer is bypassed (e.g., a firewall rule is somehow removed), the other layers continue to restrict the sandboxed process. The `WRITE_RESTRICTED` token flag is particularly powerful because it makes the Windows kernel itself enforce write restrictions at the syscall level - no amount of userspace trickery can bypass it.
