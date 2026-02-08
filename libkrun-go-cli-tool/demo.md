# krunsh: Pipe Shell Commands to an Ephemeral libkrun MicroVM

*2026-02-08T01:22:46Z*

krunsh is a Go CLI tool built with [libkrun-go](https://github.com/mishushakov/libkrun-go) bindings for [libkrun](https://github.com/containers/libkrun). It reads newline-delimited shell commands from stdin and executes them inside an ephemeral microVM that is discarded when the commands finish. No temporary files are needed.

## Building the tool

```bash
ldconfig -p | grep -E 'libkrun|libkrunfw'
```

```output
	libkrunfw.so.5 (libc6,x86-64) => /usr/local/lib64/libkrunfw.so.5
	libkrunfw.so (libc6,x86-64) => /usr/local/lib64/libkrunfw.so
	libkrun.so.1 (libc6,x86-64) => /usr/local/lib64/libkrun.so.1
	libkrun.so (libc6,x86-64) => /usr/local/lib64/libkrun.so
```

```bash
CGO_LDFLAGS="-L/usr/local/lib64" GOTOOLCHAIN=local go build -v -o krunsh . 2>&1; echo "Binary: $(ls -lh krunsh | awk '{print $5}')"
```

```output
Binary: 2.5M
```

## Usage

```bash
LD_LIBRARY_PATH=/usr/local/lib64 ./krunsh --help 2>&1 || true
```

```output
Usage of ./krunsh:
  -cpus uint
    	number of vCPUs (default 1)
  -mem uint
    	RAM in MiB (default 256)
  -root string
    	root filesystem path for the VM (default "/")
  -shell string
    	shell to use for executing commands (default "/bin/sh")
  -verbose
    	enable libkrun info logging
  -workdir string
    	working directory inside the VM (default "/")
```

## KVM pre-flight check

This environment runs under gVisor (no /dev/kvm). The tool detects this cleanly:

```bash
printf 'echo hello world\nuname -a\nwhoami\n' | LD_LIBRARY_PATH=/usr/local/lib64 ./krunsh 2>&1 || true
```

```output
krunsh: /dev/kvm not found - KVM is required for libkrun microVMs.
  Ensure you are running on a Linux host with KVM enabled.
  In containers, pass through /dev/kvm (e.g. docker run --device /dev/kvm ...)
```

## Proof of execution via QEMU nested virtualization

To prove krunsh actually works, we used QEMU TCG (software CPU emulation) to boot a real Linux kernel with KVM support inside this gVisor sandbox. The kvm_amd module loads in QEMU's emulated AMD CPU, providing a working /dev/kvm.

The full QEMU test output is saved in qemu-test-output.txt. Here are the key results:

```bash
cat qemu-test-output.txt | sed -n '/=== Test 2/,$p'
```

```output
=== Test 2: single echo via krunsh ===
krunsh: running 1 command(s) in microVM (cpus=1, mem=128MiB, root=/)
hello-from-microvm
[2026-02-08T01:16:23Z INFO  vmm] Vmm is stopping.
exit: 0

=== Test 3: multiple commands via krunsh ===
krunsh: running 3 command(s) in microVM (cpus=1, mem=128MiB, root=/)
hello world
Linux localhost 6.12.68 #1 SMP PREEMPT_DYNAMIC Mon Feb  2 09:49:04 CET 2026 x86_64 Linux
root
[2026-02-08T01:16:28Z INFO  vmm] Vmm is stopping.
exit: 0

=== All tests done ===
[   18.060757] reboot: Power down
```

The kernel version **6.12.68** in the uname output is from libkrunfw's built-in kernel, proving the commands ran inside the microVM (not on the QEMU host which runs 6.12.13, nor the gVisor host which runs 4.4.0). Three levels of nested virtualization: gVisor -> QEMU TCG -> KVM (kvm_amd) -> libkrun microVM.

## Source code

```bash
cat main.go
```

```output
// krunsh reads newline-delimited shell commands from stdin and executes them
// inside an ephemeral libkrun microVM. The VM is discarded when the commands
// finish (or stdin is closed).
//
// Usage:
//
//	echo -e "echo hello\nuname -a" | krunsh
//	krunsh < commands.txt
//	krunsh --cpus 2 --mem 512 --root /
package main

import (
	"bufio"
	"flag"
	"fmt"
	"os"
	"strings"

	"krunsh/krun"
)

func main() {
	cpus := flag.Uint("cpus", 1, "number of vCPUs")
	mem := flag.Uint("mem", 256, "RAM in MiB")
	root := flag.String("root", "/", "root filesystem path for the VM")
	workdir := flag.String("workdir", "/", "working directory inside the VM")
	shell := flag.String("shell", "/bin/sh", "shell to use for executing commands")
	verbose := flag.Bool("verbose", false, "enable libkrun info logging")
	flag.Parse()

	// Read commands from stdin.
	var commands []string
	scanner := bufio.NewScanner(os.Stdin)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line != "" && !strings.HasPrefix(line, "#") {
			commands = append(commands, line)
		}
	}
	if err := scanner.Err(); err != nil {
		fmt.Fprintf(os.Stderr, "krunsh: reading stdin: %v\n", err)
		os.Exit(1)
	}

	if len(commands) == 0 {
		fmt.Fprintf(os.Stderr, "krunsh: no commands provided on stdin\n")
		flag.Usage()
		os.Exit(1)
	}

	if err := run(commands, *cpus, *mem, *root, *workdir, *shell, *verbose); err != nil {
		fmt.Fprintf(os.Stderr, "krunsh: %v\n", err)
		os.Exit(1)
	}
}

func checkKVM() error {
	info, err := os.Stat("/dev/kvm")
	if err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("/dev/kvm not found - KVM is required for libkrun microVMs.\n" +
				"  Ensure you are running on a Linux host with KVM enabled.\n" +
				"  In containers, pass through /dev/kvm (e.g. docker run --device /dev/kvm ...)")
		}
		return fmt.Errorf("/dev/kvm: %w", err)
	}
	// Check it's a character device.
	if info.Mode()&os.ModeCharDevice == 0 {
		return fmt.Errorf("/dev/kvm exists but is not a character device")
	}
	return nil
}

func run(commands []string, cpus, mem uint, root, workdir, shell string, verbose bool) error {
	// Pre-flight check for KVM support.
	if err := checkKVM(); err != nil {
		return err
	}

	if verbose {
		if err := krun.SetLogLevel(krun.LogLevelInfo); err != nil {
			return fmt.Errorf("set log level: %w", err)
		}
	}

	// Build a single command string to pass via sh -c.
	// Commands are joined with && so execution stops on first failure.
	// This avoids needing a temp file visible to the VM's rootfs.
	cmdStr := strings.Join(commands, " && ")

	fmt.Fprintf(os.Stderr, "krunsh: running %d command(s) in microVM (cpus=%d, mem=%dMiB, root=%s)\n",
		len(commands), cpus, mem, root)

	// Create a new VM context.
	ctx, err := krun.CreateContext()
	if err != nil {
		return fmt.Errorf("create context: %w", err)
	}

	if err := ctx.SetVMConfig(uint8(cpus), uint32(mem)); err != nil {
		return fmt.Errorf("set vm config: %w", err)
	}

	if err := ctx.SetRoot(root); err != nil {
		return fmt.Errorf("set root: %w", err)
	}

	if err := ctx.SetWorkdir(workdir); err != nil {
		return fmt.Errorf("set workdir: %w", err)
	}

	// Execute the script inside the VM using the chosen shell.
	// Note: libkrun's krun_set_exec argv is NOT the same as execve argv.
	// The exec path specifies the binary. The argv elements are passed as
	// command-line arguments (i.e., argv[0] here becomes $1 inside the
	// process, NOT the process name). So we pass ["-c", cmdStr] rather
	// than ["sh", "-c", cmdStr].
	env := []string{
		"PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
		"HOME=/root",
		"TERM=xterm-256color",
	}
	if err := ctx.SetExec(shell, []string{"-c", cmdStr}, env); err != nil {
		return fmt.Errorf("set exec: %w", err)
	}

	// StartEnter does not return on success - the process exits with
	// the guest workload's exit code.
	if err := ctx.StartEnter(); err != nil {
		return fmt.Errorf("start vm: %w", err)
	}

	return nil
}
```

## Binary details

```bash
file ./krunsh && ls -lh ./krunsh && ldd ./krunsh
```

```output
./krunsh: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), dynamically linked, interpreter /lib64/ld-linux-x86-64.so.2, BuildID[sha1]=3c54cea825c4ef9824258c9233ef0d92392c8201, for GNU/Linux 3.2.0, with debug_info, not stripped
-rwxr-xr-x 1 root root 2.5M Feb  8 01:46 ./krunsh
	linux-vdso.so.1 (0x00007eceb7a94000)
	libkrun.so.1 => /usr/local/lib64/libkrun.so.1 (0x00007eceb7600000)
	libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007eceb7200000)
	libgcc_s.so.1 => /lib/x86_64-linux-gnu/libgcc_s.so.1 (0x00007eceb7a59000)
	/lib64/ld-linux-x86-64.so.2 (0x000055ead07d6000)
```
