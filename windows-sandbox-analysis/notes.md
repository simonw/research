# Windows Sandbox Analysis - Research Notes

## 2026-03-05: Initial exploration

### Repository cloned
- Cloned openai/codex from GitHub to /tmp/codex
- Target crate: `codex-rs/windows-sandbox-rs`

### File inventory
The crate has ~25 source files. Key modules:
- `lib.rs` - Main entry point with two implementations: `windows_impl` (legacy inline) and `elevated_impl` (newer, uses separate sandbox user)
- `token.rs` - Windows restricted token creation using `CreateRestrictedToken`
- `acl.rs` - DACL/ACL manipulation (allow/deny ACEs)
- `firewall.rs` - Windows Firewall rule management for network blocking
- `sandbox_users.rs` - Creates dedicated Windows local user accounts
- `identity.rs` - Credential management (DPAPI-encrypted passwords)
- `policy.rs` - Sandbox policy parsing (ReadOnly, WorkspaceWrite, etc.)
- `process.rs` - `CreateProcessAsUserW` wrapper
- `setup_orchestrator.rs` - Elevated setup flow (UAC prompt)
- `setup_main_win.rs` - The actual elevated setup binary logic
- `elevated_impl.rs` - Newer sandbox implementation using `CreateProcessWithLogonW`
- `command_runner_win.rs` - Command runner binary that runs inside the sandbox user session
- `allow.rs` - Computes allow/deny path sets from policy
- `audit.rs` - World-writable directory scanner
- `cap.rs` - Capability SID generation and persistence
- `env.rs` - Environment variable manipulation for sandboxing
- `dpapi.rs` - DPAPI encrypt/decrypt for credential storage
- `hide_users.rs` - Hides sandbox users from Windows login screen
- `workspace_acl.rs` - Protects .codex and .agents directories
- `helper_materialization.rs` - Copies helper binaries to sandbox-accessible locations
- `cwd_junction.rs` - NTFS junction workaround for CWD access
- `read_acl_mutex.rs` - Named mutex for coordinating read ACL setup
- `path_normalization.rs` - Case-insensitive path canonicalization

### Key findings

1. **Two sandbox architectures**: Legacy (inline restricted token) and newer (separate user logon)
2. **Five-layer security model**: User isolation, restricted tokens, ACL enforcement, firewall rules, environment poisoning
3. **The sandbox does NOT use Windows Sandbox (WSB)** - it uses native Windows security primitives
4. **Capability SIDs** are randomly generated S-1-5-21 format SIDs used as "tags" for ACL grants
5. **DPAPI with machine scope** used for credential storage so both elevated and non-elevated processes can decrypt
6. **Per-workspace isolation** using unique capability SIDs keyed by canonicalized CWD path

### Architecture observations
- The elevated setup binary (`codex-windows-sandbox-setup.exe`) runs with admin privileges to:
  - Create sandbox user accounts
  - Set up firewall rules
  - Apply ACLs to read/write roots
- The command runner (`codex-command-runner.exe`) runs as the sandbox user and:
  - Creates a restricted token from the sandbox user's token
  - Spawns the actual command under that restricted token
  - Communicates via named pipes back to the parent process
- The main Codex process orchestrates everything and uses `CreateProcessWithLogonW` to launch the runner as the sandbox user
