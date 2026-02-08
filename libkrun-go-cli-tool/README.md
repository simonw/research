# krunsh

**A Go CLI tool that pipes shell commands into an ephemeral microVM.**

You pipe newline-delimited shell commands on stdin, krunsh spins up a
[libkrun](https://github.com/containers/libkrun) microVM, runs them, prints the
output, and tears down the VM. One invocation, zero persistent state.

```
$ printf 'echo hello\nuname -a\nwhoami\n' | krunsh
hello
Linux localhost 6.12.68 #1 SMP PREEMPT_DYNAMIC ... x86_64 Linux
root
```

The kernel version above (`6.12.68`) is from libkrunfw's built-in kernel — proof
the commands ran inside the microVM, not on the host.

## Background

[libkrun](https://github.com/containers/libkrun) is a Rust library (by the
Containers project) that creates lightweight KVM-based microVMs. It uses
[libkrunfw](https://github.com/containers/libkrunfw) for a minimal Linux
kernel and [virtiofs](https://virtio-fs.gitlab.io/) to share the host filesystem
into the guest. This project uses the
[libkrun-go](https://github.com/mishushakov/libkrun-go) Go/CGO bindings to
drive libkrun from a small CLI.

## How it works

1. Reads newline-delimited shell commands from stdin (blank lines and `#` comments are skipped)
2. Joins commands with `&&` into a single string (stops on first failure)
3. Creates a libkrun microVM context with configurable CPUs, RAM, and root filesystem
4. Executes `/bin/sh -c "cmd1 && cmd2 && ..."` inside the VM
5. The process exits with the VM guest's exit code; the VM is ephemeral and discarded

No temporary files are created — commands are passed directly via libkrun's
exec API.

## Requirements

- Linux with KVM support (`/dev/kvm` must be available)
- [libkrun](https://github.com/containers/libkrun) shared library installed
- [libkrunfw](https://github.com/containers/libkrunfw) firmware library installed
- Go 1.22+ for building

## Building

### Install dependencies

```bash
# Install libkrunfw (pre-built, from GitHub releases)
curl -L -o /tmp/libkrunfw-x86_64.tgz \
  https://github.com/containers/libkrunfw/releases/download/v5.2.0/libkrunfw-x86_64.tgz
tar xzf /tmp/libkrunfw-x86_64.tgz -C /usr/local/
ldconfig

# Build and install libkrun from source (requires Rust and libclang-dev)
git clone --recurse-submodules https://github.com/mishushakov/libkrun-go
cd libkrun-go/libkrun
make && sudo make install
ldconfig
```

### Build krunsh

```bash
CGO_LDFLAGS="-L/usr/local/lib64" go build -o krunsh .
```

## Usage

```
echo -e "echo hello\nuname -a" | ./krunsh
```

```
cat commands.txt | ./krunsh --cpus 2 --mem 512
```

### Flags

| Flag | Default | Description |
|------|---------|-------------|
| `-cpus` | 1 | Number of vCPUs |
| `-mem` | 256 | RAM in MiB |
| `-root` | `/` | Root filesystem path for the VM |
| `-workdir` | `/` | Working directory inside the VM |
| `-shell` | `/bin/sh` | Shell to use for executing commands |
| `-verbose` | false | Enable libkrun info logging |

## Testing: QEMU nested virtualization

This project was developed in a gVisor sandbox (no `/dev/kvm`). To prove it
actually works, we booted QEMU in TCG (software emulation) mode with an Alpine
Linux kernel. Inside QEMU, the `kvm_amd` module loads — the emulated `qemu64`
CPU looks like AMD, so KVM works via nested virtualization. This gave us three
levels of virtualization:

| Level | Kernel | Role |
|-------|--------|------|
| gVisor sandbox | 4.4.0 | Development environment |
| QEMU TCG VM | 6.12.13-0-virt (Alpine) | Provides `/dev/kvm` via kvm_amd |
| libkrun microVM | 6.12.68 (libkrunfw) | Runs the actual commands |

The live test output (from [demo.md](demo.md)):

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

## Key API discovery

libkrun's `krun_set_exec` argv semantics differ from POSIX execve. The argv
elements are passed as command-line arguments, not as the process's argv array.
So `argv[0]` becomes `$1`, not `$0`. You need:

```go
// Correct: "-c" becomes $1, cmdStr becomes $2
ctx.SetExec("/bin/sh", []string{"-c", cmdStr}, env)

// Wrong: "/bin/sh" becomes $1, sh tries to source itself as a script
ctx.SetExec("/bin/sh", []string{"/bin/sh", "-c", cmdStr}, env)
```

See [notes.md](notes.md) for the full debugging story.

## Limitations

- Requires KVM — will not work in containers without `/dev/kvm` passthrough
  or in sandboxed environments like gVisor
- `StartEnter()` replaces the calling process (calls `exit()` on completion),
  so the tool runs one batch of commands per invocation
- The host filesystem is used as the VM's root by default; use `--root` to
  point at an alternative rootfs for stronger isolation

## Files

- `main.go` — CLI entry point and VM orchestration (~130 lines)
- `krun/` — Go bindings for libkrun (copied from libkrun-go)
- `libkrun/include/` — libkrun C header for CGO compilation
- `demo.md` — Showboat demo: builds tool, runs QEMU test live
- `notes.md` — Full investigation notes
- `qemu-test-output.txt` — Raw QEMU test output
