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
- `SetExec()` sets the initial process - we use `/bin/sh` with a temp script
- Environment variables must be kept minimal - inheriting the full host env
  can exceed the kernel command line limit on aarch64 (2048 bytes)
- Without libkrunfw, the error is: "Couldn't find or load libkrunfw.so.5"
- Without /dev/kvm, libkrun panics in Rust code:
  "Error creating the Kvm object: Error(2)" (ENOENT)

## Tool design decisions

- Read all commands from stdin first, then create a temp shell script
- Skip blank lines and comment lines (starting with #)
- Use `set -e` in the generated script so commands stop on first failure
- Pre-flight check for /dev/kvm before calling into libkrun (avoids Rust panic)
- Configurable: --cpus, --mem, --root, --workdir, --shell, --verbose
- The temp script directory is cleaned up via `defer os.RemoveAll()`
  (though if StartEnter succeeds, the deferred function never runs since
  the process exits from within C/Rust code)

## What works

- libkrun + libkrunfw build and install correctly
- Go bindings compile and link fine with Go 1.24
- The krunsh binary is 2.5MB, dynamically linked against libkrun.so.1
- All configuration steps succeed (CreateContext, SetVMConfig, SetRoot, etc.)
- Clean error handling for missing KVM

## What doesn't work (in this environment)

- Cannot actually start VMs because /dev/kvm is not available
- This is a gVisor sandbox - no hardware virtualization passthrough
- Would need bare metal or a VM with nested virt + /dev/kvm passed through
