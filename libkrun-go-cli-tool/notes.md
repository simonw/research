# Investigation Notes: libkrun-go CLI Tool

## Goal

Build a Go CLI tool that reads newline-delimited shell commands from stdin and
executes them inside an ephemeral libkrun microVM.

## Environment Discovery

- Platform: Linux x86_64, gVisor sandbox (runsc kernel 4.4.0)
- Go: 1.24.7 (`/usr/local/go/bin/go`)
- Rust: 1.93.0 (needed to build libkrun from source)
- **No /dev/kvm** - this is the fundamental blocker for actually running VMs

## Building libkrun from source

### libkrunfw (firmware)

- libkrunfw is a separate library containing a minimal Linux kernel used as the
  VM firmware
- Building it from source requires downloading and compiling a full Linux kernel
  (linux-6.12.68) - very heavy
- Pre-built binaries available from GitHub releases:
  `https://github.com/containers/libkrunfw/releases`
- Downloaded `libkrunfw-x86_64.tgz` (v5.2.0, ~6.6MB) which contains
  `libkrunfw.so.5.2.0` and symlinks
- Installed to `/usr/local/lib64/`

### libkrun (VM library)

- Source cloned via git submodule in the libkrun-go repo
- Required `libclang-dev` to be installed (`apt-get install -y libclang-dev`)
- Built successfully with just `make` in the libkrun directory
- Produces `libkrun.so.1.17.2` (~Rust-based, uses KVM under the hood)
- Installed with `make install` to `/usr/local/lib64/`
- Added `/usr/local/lib64` to ldconfig path

### libkrun-go (Go bindings)

- The repo's go.mod declares `go 1.25.5` but the code only actually needs
  Go 1.22+ features (`range n` syntax, `unsafe.Slice`)
- Created our own Go module with `go 1.24.7` and copied the `krun/` package
  directly - works fine
- CGO flags: `#cgo CFLAGS: -I${SRCDIR}/../libkrun/include` and
  `#cgo LDFLAGS: -lkrun`
- Need `CGO_LDFLAGS="-L/usr/local/lib64"` when building since libkrun is
  installed there

## libkrun API observations

- `StartEnter()` does not return on success - it calls `exit()` with the
  guest workload's exit code. This is a fundamental design choice.
- The VM uses a host directory as root filesystem (via `SetRoot()`)
- `SetExec()` sets the initial process to run in the VM
- Environment variables must be kept minimal - inheriting the full host env
  can exceed the kernel command line limit on aarch64 (2048 bytes)
- Without libkrunfw, the error is: "Couldn't find or load libkrunfw.so.5"
- Without /dev/kvm, libkrun panics in Rust code:
  "Error creating the Kvm object: Error(2)" (ENOENT)

## Critical API discovery: krun_set_exec argv semantics

**libkrun's `krun_set_exec` argv is NOT the same as execve's argv.**

In POSIX execve, argv[0] is the program name. But in libkrun, the exec path
specifies the binary, and the argv elements are passed as command-line
arguments. So argv[0] becomes `$1` inside the process, not `$0`.

This means:
- **Wrong**: `SetExec("/bin/sh", ["/bin/sh", "-c", cmd], env)` — this
  makes sh try to source `/bin/sh` (the binary itself) as a script, producing
  ELF binary interpretation errors
- **Wrong**: `SetExec("/bin/sh", ["sh", "-c", cmd], env)` — this makes
  sh try to open a file called "sh"
- **Correct**: `SetExec("/bin/sh", ["-c", cmd], env)` — this correctly
  runs `/bin/sh -c "command"`

Evidence from debugging:
- argv=`["/bin/sh", "-c", "echo hello"]` → `/bin/sh: line 0: ~~: not found` (ELF bytes)
- argv=`["sh", "-c", "echo hello"]` → `/bin/sh: can't open 'sh': No such file`
- argv=`["-c", "echo hello"]` → `hello` (correct!)

## Tool design decisions

- Read all commands from stdin, join with ` && `, pass via `sh -c`
- Skip blank lines and comment lines (starting with #)
- Commands joined with `&&` so execution stops on first failure
- No temporary files needed - commands passed directly via libkrun's SetExec API
- Pre-flight check for /dev/kvm before calling into libkrun (avoids Rust panic)
- Configurable: --cpus, --mem, --root, --workdir, --shell, --verbose

### Why no temp files?

Initially tried writing commands to a temp shell script, but the temp file on
the host filesystem is not visible inside the libkrun microVM's virtiofs mount
in the same location. The `sh -c "cmd1 && cmd2"` approach avoids this entirely
and requires no filesystem coordination between host and guest.

### Earlier approach: temp file issues

The initial implementation wrote commands to a temp file and passed it as the
exec argument. But virtiofs in libkrun exposes the root directory as the VM's
filesystem. A temp file created in `/tmp/krunsh-xxx/` on the host would need to
exist at that exact path in the VM's view. This turned out to be unreliable,
especially when testing inside QEMU where the rootfs is an initramfs.

## Attempts to get KVM working

### Docker (failed)

- Started Docker daemon with `--iptables=false --bridge=none --storage-driver=vfs`
  (workarounds for gVisor's limited syscall support)
- Containers still run under gVisor (same kernel 4.4.0)
- Created `/dev/kvm` via mknod — device node exists but KVM ioctls return ENXIO
- Conclusion: Docker in gVisor doesn't escape the sandbox

### QEMU TCG emulation (succeeded!)

- Installed `qemu-system-x86_64` (version 8.2.2)
- Downloaded Alpine Linux virt ISO for its kernel (vmlinuz-virt) and modules
- Ran QEMU in TCG (software CPU emulation) mode — no KVM needed on the host!
- Inside QEMU: real Linux kernel 6.12.13-0-virt boots
- Key discovery: `kvm_amd` module loads with nested virtualization in TCG mode!
  - `kvm_intel` fails ("VMX not supported by CPU 0")
  - But the default QEMU cpu (`qemu64`) emulates an AMD-like CPU
  - `kvm_amd` loads successfully: "Nested Virtualization enabled"
  - `/dev/kvm` appears and is functional!

### Building the QEMU test environment

1. Created Alpine rootfs from Docker image (`alpine:3.21`)
2. Copied into rootfs:
   - krunsh binary (statically linked to Go runtime, dynamically to libkrun)
   - libkrun.so.1 and libkrunfw.so.5 libraries
   - glibc runtime (libc.so.6, ld-linux-x86-64.so.2, libpthread, etc.) since
     krunsh is glibc-linked but Alpine uses musl
   - KVM kernel modules (kvm.ko, kvm-amd.ko) from the Alpine kernel package
3. Created init.sh script that:
   - Mounts proc/sys/dev filesystems
   - Redirects output to serial console (`exec >/dev/ttyS0 2>&1`)
   - Loads KVM modules via insmod
   - Runs krunsh tests
4. Built initramfs with: `cd rootfs && find . | cpio -o -H newc | gzip > initramfs.cpio.gz`
5. Booted with: `qemu-system-x86_64 -kernel vmlinuz-virt -initrd initramfs.cpio.gz -append "console=ttyS0 rdinit=/init.sh loglevel=1" -m 2048 -nographic -no-reboot`

### Serial console output issues

- Initially, output from init.sh wasn't visible on the serial console
- The kernel `quiet` flag suppressed all output including init script output
- Fixed by removing `quiet` and using `loglevel=1` to reduce kernel noise
- Still needed `exec >/dev/ttyS0 2>&1` in init.sh to redirect script output
  to the serial port (otherwise output goes to /dev/console which isn't the
  serial port in this setup)

### Shell / symlink issues in virtiofs

- `/bin/sh` in Alpine rootfs is a symlink to `/bin/busybox`
- Replaced symlink with hardlink (same inode) for more reliable resolution
- But the main fix was understanding the argv semantics (see above)

### Busybox argv[0] applet detection

- Busybox uses argv[0] to determine which applet to run
- `/bin/busybox -c "cmd"` → `-c: applet not found` (doesn't recognize -c as applet)
- `/bin/sh -c "cmd"` → runs the `sh` applet which handles `-c` correctly
- This was a red herring — the real issue was argv semantics (see above)

## What works (PROVEN!)

- libkrun + libkrunfw build and install correctly
- Go bindings compile and link fine with Go 1.24
- The krunsh binary is ~2.5MB, dynamically linked against libkrun.so.1
- All configuration steps succeed (CreateContext, SetVMConfig, SetRoot, etc.)
- Clean error handling for missing KVM
- **Successfully runs commands inside a microVM!** (verified via QEMU nested virt)

### Proof of successful execution

Inside a QEMU TCG-emulated VM with KVM modules loaded:

```
=== Test 2: single echo via krunsh ===
krunsh: running 1 command(s) in microVM (cpus=1, mem=128MiB, root=/)
hello-from-microvm
exit: 0

=== Test 3: multiple commands via krunsh ===
krunsh: running 3 command(s) in microVM (cpus=1, mem=128MiB, root=/)
hello world
Linux localhost 6.12.68 #1 SMP PREEMPT_DYNAMIC Mon Feb  2 09:49:04 CET 2026 x86_64 Linux
root
exit: 0
```

Note the kernel versions proving three levels of nested virtualization:
- Host (gVisor sandbox): Linux 4.4.0
- QEMU VM (Alpine kernel): Linux 6.12.13-0-virt
- libkrun microVM (libkrunfw kernel): Linux 6.12.68

Three levels: gVisor → QEMU TCG → KVM (via kvm_amd in TCG) → libkrun microVM.

## What doesn't work (in this environment directly)

- Cannot start VMs directly because gVisor doesn't expose /dev/kvm
- Need QEMU TCG as an intermediary to get KVM support
- Docker in gVisor also doesn't provide KVM
