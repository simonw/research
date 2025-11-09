OpenAI Codex CLI's sandbox employs strong, platform-specific isolation to securely constrain the behavior of AI-driven code agents. On macOS, it uses Apple's Seatbelt sandbox with finely tuned dynamic policies, while on Linux, it combines Landlock for strict filesystem controls and seccomp for syscall-based network blocking—ensuring that agents can only write to user-approved directories and have no outgoing network by default. Both platforms feature special protection for `.git` repositories, path canonicalization to thwart symlink attacks, and enforce least-privilege principles, all integrated with user-configurable approval policies for flexibility. Key tools include the [OpenAI Codex CLI](https://github.com/openai/codex) and related [sandbox documentation](https://github.com/openai/codex/blob/main/docs/sandbox.md).

**Key findings:**
- Codex's sandbox distinguishes between DangerFullAccess (no sandbox), ReadOnly, and WorkspaceWrite modes, tightly controlling file/network access.
- Version control metadata (e.g., `.git`) is always read-only, preventing AI from corrupting repositories.
- Linux sandboxing requires kernel 5.13+ for Landlock; macOS relies on built-in Seatbelt.
- All attempted network operations are blocked at the syscall level, except for local IPC.
- Windows support is experimental; robust isolation is limited outside macOS/Linux.
