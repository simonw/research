Krunsh is a minimal Go CLI tool that executes newline-delimited shell commands inside an ephemeral KVM-based microVM, leveraging the [libkrun](https://github.com/containers/libkrun) library for lightweight virtualization. By piping commands from stdin, krunsh spins up a microVM, runs the specified commands using `/bin/sh -c`, captures the output, and discards the VM afterward, ensuring zero persistent state and strong process isolation. The tool is built upon [libkrun-go](https://github.com/mishushakov/libkrun-go), allowing configurable VMs (CPUs, RAM, root filesystem) and requires a Linux host with KVM support. Extensive nested virtualization tests (including QEMU TCG scenarios) confirm that commands are executed entirely within the microVM environment, not on the host.

**Key highlights:**
- Krunsh creates microVMs per invocation, guaranteeing ephemeral execution and no persistent artifacts.
- Uses host filesystem sharing via virtiofs but allows specifying alternate root filesystems for greater isolation.
- Requires direct KVM access, limiting use in containers or sandboxes without `/dev/kvm`.
- Demonstrates correct CLI-to-libkrun API usage for POSIX shell execution semantics.
