# krunsh: Pipe Shell Commands to an Ephemeral libkrun MicroVM

*2026-02-08T00:07:26Z*

krunsh is a Go CLI tool built with [libkrun-go](https://github.com/mishushakov/libkrun-go) bindings for [libkrun](https://github.com/containers/libkrun). It reads newline-delimited shell commands from stdin and executes them inside an ephemeral microVM that is discarded when the commands finish.

## Building the tool

First, let's verify libkrun and libkrunfw are installed:

```bash
ldconfig -p | grep -E 'libkrun|libkrunfw'
```

```output
	libkrunfw.so.5 (libc6,x86-64) => /usr/local/lib64/libkrunfw.so.5
	libkrunfw.so (libc6,x86-64) => /usr/local/lib64/libkrunfw.so
	libkrun.so.1 (libc6,x86-64) => /usr/local/lib64/libkrun.so.1
	libkrun.so (libc6,x86-64) => /usr/local/lib64/libkrun.so
```

Now let's build krunsh from source:

```bash
CGO_LDFLAGS="-L/usr/local/lib64" GOTOOLCHAIN=local go build -v -o krunsh . 2>&1
```

```output
```

## Usage

The tool accepts newline-delimited commands on stdin and has several flags for configuring the VM:

```bash
LD_LIBRARY_PATH=/usr/local/lib64 /home/user/research/libkrun-go-cli-tool/krunsh --help 2>&1 || true
```

```output
Usage of /home/user/research/libkrun-go-cli-tool/krunsh:
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

When no commands are provided, the tool shows usage info:

```bash
echo '' | LD_LIBRARY_PATH=/usr/local/lib64 /home/user/research/libkrun-go-cli-tool/krunsh 2>&1 || true
```

```output
krunsh: no commands provided on stdin
Usage of /home/user/research/libkrun-go-cli-tool/krunsh:
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

## KVM requirement

libkrun requires KVM for hardware virtualization. This demo environment runs under gVisor (a sandbox kernel), which doesn't expose /dev/kvm. The tool detects this and provides a clear error message:

```bash
printf 'echo hello world\nuname -a\nwhoami\n' | LD_LIBRARY_PATH=/usr/local/lib64 /home/user/research/libkrun-go-cli-tool/krunsh 2>&1 || true
```

```output
krunsh: /dev/kvm not found - KVM is required for libkrun microVMs.
  Ensure you are running on a Linux host with KVM enabled.
  In containers, pass through /dev/kvm (e.g. docker run --device /dev/kvm ...)
```

On a system with KVM available (e.g. a bare-metal Linux host or a VM with nested virtualization), the above commands would execute inside an ephemeral microVM and the VM would be discarded afterward.

## Source code

Here's the main.go source:

```bash
cat /home/user/research/libkrun-go-cli-tool/main.go
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
	"path/filepath"
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

	// Write the commands to a temporary script file that the VM can execute.
	script := "#!/bin/sh\nset -e\n" + strings.Join(commands, "\n") + "\n"
	tmpDir, err := os.MkdirTemp("", "krunsh-*")
	if err != nil {
		return fmt.Errorf("create temp dir: %w", err)
	}
	defer os.RemoveAll(tmpDir)

	scriptPath := filepath.Join(tmpDir, "run.sh")
	if err := os.WriteFile(scriptPath, []byte(script), 0755); err != nil {
		return fmt.Errorf("write script: %w", err)
	}

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
	env := []string{
		"PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
		"HOME=/root",
		"TERM=xterm-256color",
	}
	if err := ctx.SetExec(shell, []string{shell, scriptPath}, env); err != nil {
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

The binary links against libkrun (which in turn loads libkrunfw at runtime). The compiled binary is quite compact:

```bash
ls -lh /home/user/research/libkrun-go-cli-tool/krunsh && file /home/user/research/libkrun-go-cli-tool/krunsh && ldd /home/user/research/libkrun-go-cli-tool/krunsh
```

```output
-rwxr-xr-x 1 root root 2.5M Feb  8 00:08 /home/user/research/libkrun-go-cli-tool/krunsh
/home/user/research/libkrun-go-cli-tool/krunsh: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), dynamically linked, interpreter /lib64/ld-linux-x86-64.so.2, BuildID[sha1]=58c0d6986fd8c8efe2813c74b00afc89a93e863e, for GNU/Linux 3.2.0, with debug_info, not stripped
	linux-vdso.so.1 (0x00007ead01bd0000)
	libkrun.so.1 => /usr/local/lib64/libkrun.so.1 (0x00007ead01800000)
	libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007ead01400000)
	libgcc_s.so.1 => /lib/x86_64-linux-gnu/libgcc_s.so.1 (0x00007ead017d2000)
	/lib64/ld-linux-x86-64.so.2 (0x00005593a9c69000)
```
