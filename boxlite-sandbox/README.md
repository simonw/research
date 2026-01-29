# BoxLite Sandbox Investigation

An investigation into [BoxLite](https://github.com/boxlite-ai/boxlite) (v0.5.8), an embeddable micro-VM runtime designed for AI agent sandboxes and multi-tenant code execution.

## What is BoxLite?

BoxLite provides lightweight VMs ("Boxes") that run OCI containers with hardware-level isolation. Unlike Docker (which uses Linux namespaces/cgroups), BoxLite runs actual virtual machines via [libkrun](https://github.com/containers/libkrun), using KVM on Linux and Hypervisor.framework on macOS.

The architecture looks like this:

```
Host Application
  └─ BoxLite Runtime (embedded Rust library)
       └─ Shim Process (sandboxed by "jailer")
            └─ libkrun VM (hardware virtualization)
                 └─ Guest Agent (gRPC server)
                      └─ OCI Container Runtime
                           └─ User code runs here
```

Each Box gets its own Linux kernel. Communication between host and guest uses gRPC over vsock.

## Environment & Running It

**Requires KVM** (Linux) or **Hypervisor.framework** (macOS Apple Silicon). The runtime checks at initialization time:

```python
# This immediately fails without /dev/kvm:
runtime = boxlite.Boxlite.default()
# → PanicException: /dev/kvm does not exist
```

Even creating a fake `/dev/kvm` device node doesn't work — BoxLite actually opens the device and validates it (ENXIO if the kernel driver isn't loaded).

The Python SDK installs cleanly via pip/uv:
```bash
uv pip install boxlite  # 31.3 MiB wheel with bundled Rust native code
```

**Our test environment** lacked KVM support (container with kernel 4.4.0, no module loading), so all findings below are from source code analysis. Test scripts are provided for running on a KVM-enabled machine.

## Python API

### Basic Usage

```python
import asyncio, boxlite

async def main():
    async with boxlite.SimpleBox(image="python:slim") as box:
        result = await box.exec("python", "-c", "print('hello')")
        print(result.stdout)  # "hello\n"
        print(result.exit_code)  # 0

asyncio.run(main())
```

### Box Types

| Type | Purpose | Default Image |
|------|---------|--------------|
| `SimpleBox` | General command execution | (user-specified) |
| `CodeBox` | Python code execution | `python:slim` |
| `BrowserBox` | Browser automation (Playwright) | `mcr.microsoft.com/playwright:v1.58.0-jammy` |
| `ComputerBox` | Desktop automation (mouse/keyboard/screenshots) | `lscr.io/linuxserver/webtop:ubuntu-xfce` |
| `InteractiveBox` | Interactive TTY sessions | (user-specified) |

### Resource Configuration

```python
boxlite.SimpleBox(
    image="python:slim",
    memory_mib=1024,           # Default: 2048
    cpus=2,                    # Default: 1
    volumes=[("/host", "/guest", "ro")],
    ports=[(8080, 80, "tcp")],
    env=[("KEY", "value")],
)
```

Memory range: 128–65536 MiB. CPU limit: up to host CPU count.

## File Access

### Inside the Container

The guest runs as root inside its own VM with a full filesystem from the OCI image. Standard file operations work normally within the guest.

### Volume Mounts

Volume mounts use virtiofs to share host directories:
- **Read-only** (`"ro"`): Guest can read but not write
- **Read-write** (`"rw"`): Guest can read and write; changes visible on host

### File Transfer

`copy_in()` and `copy_out()` transfer files via TAR archives over gRPC:

```python
await box.copy_in("/host/file.txt", "/container/file.txt")
await box.copy_out("/container/output.txt", "/host/output.txt")
```

### Host Filesystem Isolation

The guest VM has its own filesystem — it cannot see the host filesystem except through explicitly configured volume mounts. This is enforced by hardware virtualization (the VM has no access to host memory/devices).

## Network Access

BoxLite uses **gvproxy** (gVisor's network stack) for VM networking:

- Virtual subnet: `192.168.127.0/24` (gateway `.1`, guest `.2`)
- Built-in DHCP, DNS, NAT
- Full outbound internet access by default
- Port forwarding for inbound connections (TCP/UDP)

Each Box gets its own virtual network. Boxes on the same host cannot directly communicate (each has its own gvproxy instance on an isolated virtual subnet).

**macOS difference**: Uses `SOCK_DGRAM` (VFKit protocol) vs Linux `SOCK_STREAM` (QEMU protocol) for the gvproxy socket.

## Memory Limits

Memory is configured via `memory_mib` parameter, which sets the VM's physical memory. The default is 2048 MiB. The guest kernel sees this as its total physical memory.

On Linux with the jailer enabled, additional memory controls exist:
- **cgroups v2**: `memory.max` and `memory.high` (set to 90% of max for throttling before OOM)
- **RLIMIT_AS**: Maximum virtual address space for the shim process

## Time Limits

**The Python SDK does not currently implement execution timeouts.** `CodeBox.run(timeout=)` exists as a parameter but is documented as "not yet implemented."

Workarounds:
1. **`asyncio.wait_for()`** on the host side (doesn't kill the guest process)
2. **`execution.kill(signal=9)`** for manual cancellation via the low-level API
3. **`RLIMIT_CPU`** in SecurityOptions (applied to the shim, not the guest process)

For reliable time limits, you'd need to combine `asyncio.wait_for()` with killing the execution or stopping the box entirely.

## How the Sandbox Works

BoxLite implements defense-in-depth with multiple independent security layers. The design is inspired by [Firecracker's jailer](https://github.com/firecracker-microvm/firecracker/blob/main/docs/jailer.md).

### Security Presets

| Preset | Jailer | Seccomp | Chroot | Privilege Drop | cgroups | rlimits |
|--------|--------|---------|--------|---------------|---------|---------|
| `development()` | Off | Off | Off | No | No | No |
| `standard()` | On | On (Linux) | Default | No | No | Default |
| `maximum()` | On | On (Linux) | On (Linux) | uid 65534 | Yes | Restrictive |
| `default()` | **Off** | **Off** | Default | No | No | Default |

Note: `SecurityOptions::default()` has `jailer_enabled: false`. The Python SDK uses the Rust defaults, so Python users get no jailer isolation unless they configure it through the Rust API.

### Linux Sandbox (9 layers)

| Layer | Mechanism | Source File | Purpose |
|-------|-----------|-------------|---------|
| 1 | **Hardware VM (KVM)** | `vmm/krun/` | Memory/CPU isolation via hardware virtualization |
| 2 | **Namespaces** | `jailer/bwrap.rs` | User, PID, IPC, UTS namespace isolation (via bubblewrap) |
| 3 | **Chroot/pivot_root** | `jailer/bwrap.rs` | Filesystem root restriction |
| 4 | **Seccomp BPF** | `jailer/seccomp.rs` | Syscall whitelist (~110 allowed for VMM profile) |
| 5 | **Privilege dropping** | `jailer/bwrap.rs` | Drop to uid/gid 65534 (nobody) |
| 6 | **cgroups v2** | `jailer/cgroup.rs` | CPU, memory, PID count limits |
| 7 | **rlimits** | `jailer/common/rlimit.rs` | NOFILE, FSIZE, NPROC, AS, CPU limits |
| 8 | **FD cleanup** | `jailer/common/fd.rs` | Close all inherited FDs > 2 |
| 9 | **Env sanitization** | `jailer/bwrap.rs` | Clear env, allowlist only PATH/HOME/RUST_LOG |

The seccomp filters are particularly interesting — they're pre-compiled to BPF bytecode at build time from JSON definitions in `resources/seccomp/*.json`, with argument-level validation (e.g., specific ioctl commands for KVM, specific mmap flag combinations).

Network namespaces are NOT used because gvproxy needs host network access for NAT.

### macOS Sandbox (5 layers)

| Layer | Mechanism | Source File | Purpose |
|-------|-----------|-------------|---------|
| 1 | **Hardware VM (HVF)** | `vmm/krun/` | Memory/CPU isolation via Hypervisor.framework |
| 2 | **sandbox-exec (Seatbelt)** | `jailer/platform/macos/` | Kernel-enforced file/network restrictions |
| 3 | **rlimits** | `jailer/common/rlimit.rs` | NOFILE, FSIZE, NPROC, AS, CPU limits |
| 4 | **FD cleanup** | `jailer/common/fd.rs` | Close all inherited FDs > 2 |
| 5 | **Env sanitization** | via sandbox-exec args | Clear env except allowlist |

The macOS sandbox policy uses **allow-default** (not deny-default) because Hypervisor.framework requires many undocumented Mach services. File access is restricted to:
- **Read**: System libraries, dyld cache, /dev/null, /dev/urandom, boxlite home, user volumes
- **Write**: /private/tmp, /private/var/tmp, /private/var/folders, box shared directory, rw volumes
- **Network**: Optional, added when `network_enabled = true` (allows all network + DNS/TLS Mach services)

### What macOS is Missing vs Linux

| Feature | Linux | macOS | Impact |
|---------|-------|-------|--------|
| Deny-default sandbox | Yes (seccomp) | No (allow-default Seatbelt) | macOS allows unlisted syscalls |
| Namespace isolation | User, PID, IPC, UTS | None | macOS shim shares host PID/user space |
| Privilege dropping | uid/gid to nobody | Not supported | macOS shim runs as current user |
| cgroups | v2 (cpu, memory, pids) | Not available | macOS has no kernel resource groups |
| Filesystem isolation | pivot_root + chroot | Seatbelt file rules | macOS relies on path-based rules |
| Process count limits | cgroups pids.max | RLIMIT_NPROC only | macOS fork bomb protection is weaker |

The threat model document (`jailer/THREAT_MODEL.md`) explicitly notes: "macOS provides weaker isolation than Linux. Production deployments requiring maximum security should use Linux."

### Shim Spawn Sequence

```
1. Pre-spawn (parent):     Set up cgroup v2 (Linux only)
2. Build command:
   - Linux:  bubblewrap wraps the shim with namespaces + mounts
   - macOS:  /usr/bin/sandbox-exec -p "policy" wraps the shim
3. Pre-exec hook (after fork, before exec):
   - Close inherited FDs
   - Apply rlimits
   - Join cgroup (Linux)
   - Write PID file
4. Post-exec (inside shim):
   - Linux: Apply seccomp BPF filter
   - macOS: Sandbox already applied at spawn
5. Shim creates VM:
   - Start gvproxy network backend
   - Configure libkrun context
   - krun_start_enter() → process takeover → VM running
```

## Test Scripts

The following test scripts are included for running on a KVM-enabled machine:

| Script | Tests |
|--------|-------|
| `test_file_access.py` | File write inside sandbox, host path access, ro/rw volume mounts, copy_in/copy_out |
| `test_network.py` | Outbound HTTP, DNS resolution, raw TCP, port forwarding, cross-box isolation |
| `test_memory.py` | Default/explicit/minimum memory, exhaustion behavior, OOM handling |
| `test_time_limits.py` | asyncio.wait_for() timeout, long-running processes, execution kill, fork bombs |

## Key Findings

1. **BoxLite requires real hardware virtualization** — it won't work in containers or VMs without nested virtualization. It's not a container runtime; it's a VM runtime.

2. **Security isolation is strong on Linux** with 9 independent layers when the jailer is enabled with maximum settings. Each layer provides protection even if others fail.

3. **macOS isolation is significantly weaker** — allow-default sandbox, no namespaces, no cgroups, no privilege dropping. The threat model acknowledges this explicitly.

4. **The Python SDK doesn't expose security configuration** — users can't select `maximum()` or `standard()` security presets from Python. The Rust-level `SecurityOptions` is not surfaced.

5. **No execution timeout support** in the Python API — the `timeout` parameter on `CodeBox.run()` is documented as "not yet implemented."

6. **Default security is minimal** — `SecurityOptions::default()` has `jailer_enabled: false`, so without explicit configuration, the shim process runs without OS-level sandboxing. The VM hardware isolation is always present, but the defense-in-depth layers are opt-in.

7. **The gvproxy networking model** means each Box gets isolated outbound internet access by default. Boxes cannot communicate with each other directly (separate virtual subnets).

8. **Memory limits map to VM physical memory** — the `memory_mib` parameter sets actual VM RAM. With the jailer enabled, cgroups and RLIMIT_AS provide additional enforcement on the host side.
