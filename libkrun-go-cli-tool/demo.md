# krunsh: Pipe Shell Commands to an Ephemeral libkrun MicroVM

*2026-02-08T01:51:26Z*

krunsh reads newline-delimited shell commands from stdin and executes them inside an ephemeral [libkrun](https://github.com/containers/libkrun) microVM. No temporary files needed.

## Build

```bash
CGO_LDFLAGS="-L/usr/local/lib64" GOTOOLCHAIN=local go build -v -o krunsh . 2>&1; file krunsh
```

```output
krunsh
krunsh: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), dynamically linked, interpreter /lib64/ld-linux-x86-64.so.2, BuildID[sha1]=49f4f18435b3a7c34268d385105c154832aaea8b, for GNU/Linux 3.2.0, with debug_info, not stripped
```

## CLI flags

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

This gVisor sandbox has no /dev/kvm. The tool detects this:

```bash
printf 'echo hello\n' | LD_LIBRARY_PATH=/usr/local/lib64 ./krunsh 2>&1 || true
```

```output
krunsh: /dev/kvm not found - KVM is required for libkrun microVMs.
  Ensure you are running on a Linux host with KVM enabled.
  In containers, pass through /dev/kvm (e.g. docker run --device /dev/kvm ...)
```

## Running krunsh inside QEMU (live test)

To get KVM, we boot a real Linux kernel inside QEMU TCG (software emulation). The kvm_amd module loads, giving us /dev/kvm. Then krunsh runs commands inside a libkrun microVM - three levels of nested virtualization.

```bash
timeout 120 qemu-system-x86_64 \
  -kernel /tmp/alpine-iso/boot/vmlinuz-virt \
  -initrd /tmp/test-initramfs11.cpio.gz \
  -append "console=ttyS0 rdinit=/init.sh loglevel=1" \
  -m 2048 -nographic -no-reboot 2>/dev/null | sed -n "/=== QEMU VM booted ===/,\$p"
```

```output
=== QEMU VM booted ===
Kernel: Linux (none) 6.12.13-0-virt #1-Alpine SMP PREEMPT_DYNAMIC 2025-02-10 21:47:33 x86_64 Linux

=== Loading KVM modules ===
SUCCESS: /dev/kvm exists!

=== Test 1: krunsh help ===
Usage of /usr/local/bin/krunsh:
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

=== Test 2: single echo via krunsh ===
krunsh: running 1 command(s) in microVM (cpus=1, mem=128MiB, root=/)
hello-from-microvm
[90m[[0m2026-02-08T01:52:39Z [32mINFO [0m vmm[90m][0m Vmm is stopping.
exit: 0

=== Test 3: multiple commands via krunsh ===
krunsh: running 3 command(s) in microVM (cpus=1, mem=128MiB, root=/)
hello world
Linux localhost 6.12.68 #1 SMP PREEMPT_DYNAMIC Mon Feb  2 09:49:04 CET 2026 x86_64 Linux
root
[90m[[0m2026-02-08T01:52:45Z [32mINFO [0m vmm[90m][0m Vmm is stopping.
exit: 0

=== All tests done ===
[   19.489849] reboot: Power down
```

Test 2 printed `hello-from-microvm` from inside a libkrun microVM. Test 3 ran three commands: the `uname -a` output shows kernel **6.12.68** (from libkrunfw), different from the QEMU host (6.12.13) and the gVisor host (4.4.0). `whoami` returned `root`. All commands exited 0.
