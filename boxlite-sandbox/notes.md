# BoxLite Sandbox Investigation Notes

## Investigation Log

### Setup

- Cloned https://github.com/boxlite-ai/boxlite into /tmp/boxlite
- Created a uv venv at /tmp/boxlite-env and installed boxlite 0.5.8 from PyPI
- The pip wheel is 31.3 MiB — it bundles native Rust code compiled via PyO3

### Attempt to Run

Tried running a basic SimpleBox test. First error:
```
Failed to check virtualization support: unsupported: /dev/kvm does not exist
```

The CPU has vmx flags but the environment doesn't have `/dev/kvm`. Tried `mknod /dev/kvm c 10 232` to create the device node — passed the existence check but failed on open:
```
/dev/kvm exists but couldn't be accessed: No such device or address (os error 6)
```

ENXIO means there's no KVM driver behind the device node. This kernel (4.4.0) in this container environment doesn't have KVM module support. **BoxLite requires actual hardware virtualization (KVM on Linux, Hypervisor.framework on macOS)** — it's not just checking for the file, it actually opens the device.

The check happens in `boxlite/src/runtime/core.rs:111` during `BoxliteRuntime::default_runtime()`.

### Key Architectural Finding

BoxLite is NOT a container runtime. It runs actual VMs via libkrun, which uses KVM (Linux) or Hypervisor.framework (macOS). Each "Box" is a lightweight VM with its own kernel. The architecture is:

```
Host App → BoxLite Runtime → Shim Process (sandboxed) → libkrun VM → Guest Agent → OCI Container
```

The shim process is necessary because `krun_start_enter()` performs process takeover (never returns). The subprocess model keeps the host application running.

### Source Code Analysis: Jailer Module

The jailer is at `boxlite/src/jailer/` and provides defense-in-depth for the shim process.

#### Linux Isolation Layers

1. **Bubblewrap** (`jailer/bwrap.rs`): Namespace isolation via bubblewrap
   - User, PID, IPC, UTS namespaces (NOT network — gvproxy needs host networking)
   - Read-only system dir mounts: /usr, /lib, /lib64, /bin, /sbin
   - Device access: /dev/kvm, /dev/net/tun
   - tmpfs for /tmp
   - `--die-with-parent`, `--new-session`, `--clearenv`

2. **Cgroups v2** (`jailer/cgroup.rs`): Resource limits
   - Controllers: cpu, memory, pids
   - Path: `/sys/fs/cgroup/boxlite/{box_id}/`
   - Rootless path: `/sys/fs/cgroup/user.slice/user-{uid}.slice/...`
   - Writes: memory.max, memory.high (90% of max), cpu.weight, cpu.max, pids.max

3. **Seccomp BPF** (`jailer/seccomp.rs`): Syscall filtering
   - Pre-compiled to BPF bytecode at build time (zero runtime overhead)
   - Three profiles: vmm (~110 syscalls), vcpu (~85), api (~65)
   - Applied via PR_SET_NO_NEW_PRIVS + SECCOMP_SET_MODE_FILTER
   - Argument-level validation (e.g., specific ioctl commands, mmap flags)
   - Filter definitions in `resources/seccomp/*.json` per architecture

4. **rlimits** (`jailer/common/rlimit.rs`): Applied in pre_exec hook
   - RLIMIT_NOFILE, RLIMIT_FSIZE, RLIMIT_NPROC, RLIMIT_AS, RLIMIT_CPU

5. **FD cleanup** (`jailer/common/fd.rs`): Close all FDs > 2
   - Linux: uses `close_range()` syscall (5.9+) with brute-force fallback
   - macOS: brute-force close to 4096

6. **Pre-exec hook** (`jailer/pre_exec.rs`): Runs after fork(), before exec()
   - Order: close FDs → apply rlimits → join cgroup → write PID file

#### macOS Isolation Layers

1. **sandbox-exec (Seatbelt)** (`jailer/platform/macos/`):
   - Policy files: base, file_read, file_write, network (all .sbpl)
   - Uses `(allow default)` — not deny-default, because Hypervisor.framework needs undocumented Mach services
   - File read: system libs, dyld cache, /dev/null, /dev/urandom
   - File write: /private/tmp, /private/var/tmp, /private/var/folders
   - Dynamic paths added for: binary dir, boxlite home, user volumes
   - Network policy only added when `security.network_enabled = true`
   - Run via: `/usr/bin/sandbox-exec -p "policy" -D BOX_SHARED_DIR=... boxlite-shim`

2. **rlimits**: Same as Linux (NOFILE, FSIZE, NPROC, AS, CPU)

3. **FD cleanup**: Brute-force close to 4096

4. **No cgroups, no privilege dropping, no namespaces, no seccomp**

### Security Configuration (options.rs)

Three presets:
- **development()**: Everything disabled, for debugging
- **standard()**: jailer + seccomp (Linux only) enabled
- **maximum()**: Everything on, uid/gid=65534 (nobody), PID namespace, all rlimits set

Default `SecurityOptions::default()` has `jailer_enabled: false`, `seccomp_enabled: false` — so out of the box, the jailer is OFF.

The `BoxOptions` struct includes a `security: SecurityOptions` field, but the Python SDK doesn't expose it — Python users get the defaults.

### VMM Architecture

- Engine factory uses `inventory` crate for compile-time registration
- Only engine: libkrun (uses KVM/HVF)
- Process: create context → set resources → add virtiofs/block devices → configure vsock → set entrypoint → `krun_start_enter()` (process takeover)
- Vsock bridges: gRPC on port 2695, ready notification on port 2696
- URI transformation: unix socket paths → vsock://port for guest agent

### Networking (gvproxy)

- Uses gvproxy (gVisor's network stack) via CGO bridge
- Virtual subnet: 192.168.127.0/24, gateway .1, guest .2
- Provides: NAT, DHCP, DNS, port forwarding
- macOS uses SOCK_DGRAM (VFKit protocol), Linux uses SOCK_STREAM (QEMU protocol)
- gvproxy instance is `Box::leak()`'d to keep alive for VM lifetime

### Python SDK Analysis

- SimpleBox: async context manager, deferred creation (box created in `__aenter__`)
- CodeBox: extends SimpleBox with `run(code)` and `install_package()`
- `exec()` collects stdout/stderr via async iterators, then calls `wait()` for exit code
- **No execution timeout support** — `CodeBox.run(timeout=)` exists but is not implemented
- File transfer via `copy_in()` / `copy_out()` using TAR archives
- Security options NOT exposed in Python API — always gets Rust defaults
- Sync API available via greenlet-based bridge

### What I Would Test (on KVM-Enabled Machine)

1. **File access**: Write inside container, read host paths, ro/rw volume mounts, copy_in/copy_out
2. **Network**: Outbound HTTP, DNS resolution, TCP connections, port forwarding, cross-box isolation
3. **Memory**: Default allocation, explicit limits, exhaustion behavior, OOM handling
4. **Time limits**: asyncio.wait_for() timeouts, execution.kill(), fork bombs
5. **Security configurations**: Test with development/standard/maximum presets via Rust API

### Observations

- BoxLite is substantially more isolated than Docker: each Box gets its own kernel via hardware virtualization
- The jailer adds OS-level sandboxing on top of the VM — defense in depth inspired by Firecracker's jailer
- macOS isolation is significantly weaker: no deny-default sandbox, no cgroups, no privilege dropping
- The Python SDK doesn't expose security configuration — users can't set security presets from Python
- No built-in execution timeout mechanism in the Python API
- The default SecurityOptions has jailer disabled — so unless explicitly configured, the shim runs without OS-level sandboxing
