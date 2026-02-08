# krunsh

A Go CLI tool that pipes newline-delimited shell commands into an ephemeral
[libkrun](https://github.com/containers/libkrun) microVM. The VM is created,
the commands run, and the VM is discarded — providing lightweight isolation
without persistent state.

Built using [libkrun-go](https://github.com/mishushakov/libkrun-go) Go bindings.

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

## Proof of execution

Tested inside a QEMU TCG-emulated VM with KVM support (nested virtualization
via kvm_amd):

```
$ printf 'echo hello-from-microvm\n' | krunsh --mem 128 --verbose
krunsh: running 1 command(s) in microVM (cpus=1, mem=128MiB, root=/)
hello-from-microvm

$ printf 'echo hello world\nuname -a\nwhoami\n' | krunsh --mem 128 --verbose
krunsh: running 3 command(s) in microVM (cpus=1, mem=128MiB, root=/)
hello world
Linux localhost 6.12.68 #1 SMP PREEMPT_DYNAMIC Mon Feb  2 09:49:04 CET 2026 x86_64 Linux
root
```

Note the kernel version `6.12.68` is from libkrunfw's built-in kernel, proving
commands ran inside the microVM, not on the host.

## Limitations

- Requires KVM — will not work in containers without `/dev/kvm` passthrough
  or in sandboxed environments like gVisor
- `StartEnter()` replaces the calling process (calls `exit()` on completion),
  so the tool runs one batch of commands per invocation
- The host filesystem is used as the VM's root by default; use `--root` to
  point at an alternative rootfs for stronger isolation

## Key API discovery

libkrun's `krun_set_exec` argv semantics differ from POSIX execve. The argv
elements are passed as command-line arguments, not as the process's argv array.
So `argv[0]` becomes `$1`, not `$0`. See [notes.md](notes.md) for details.

## Demo

See [demo.md](demo.md) for a showboat demo document.

## Files

- `main.go` — CLI entry point and VM orchestration
- `krun/` — Go bindings for libkrun (copied from libkrun-go)
- `libkrun/include/` — libkrun C header for CGO compilation
- `demo.md` — Showboat demo document
- `notes.md` — Investigation notes
- `qemu-test-output.txt` — Full output from QEMU nested virtualization test
