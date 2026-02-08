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
