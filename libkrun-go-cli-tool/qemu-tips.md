# QEMU Tips for Agents in gVisor / Sandboxed Environments

This document captures hard-won knowledge about using QEMU inside a gVisor
(runsc) sandbox environment, where `/dev/kvm` is not available and the kernel
reports as Linux 4.4.0. These tips should help future agents who need real
kernel features, KVM, or a less-restricted Linux environment.

## What you're dealing with

The Claude Code cloud environment runs under gVisor, a userspace kernel
reimplementation. It intercepts syscalls and emulates them. Key constraints:

- **No `/dev/kvm`** — gVisor doesn't expose hardware virtualization
- **Kernel 4.4.0** — not the real kernel version; it's what gVisor reports via `uname`
- **Limited syscalls** — some Linux syscalls are unimplemented or partially implemented
- **Docker runs under gVisor too** — containers launched from Docker still use the gVisor kernel, so Docker does NOT escape the sandbox

## QEMU TCG: your escape hatch

QEMU can run in **TCG mode** (Tiny Code Generator), which is pure software CPU
emulation. No KVM needed on the host. This gives you a real Linux kernel with
full syscall support.

### Installing QEMU

```bash
apt-get update && apt-get install -y qemu-system-x86_64
```

This gets you QEMU 8.2.2 on Ubuntu 24.04.

### Getting a kernel

The easiest approach is to grab Alpine Linux's virt kernel:

```bash
# Download Alpine virt ISO (~60MB, minimal)
curl -L -o /tmp/alpine-virt.iso \
  https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/alpine-virt-3.21.3-x86_64.iso

# Extract the kernel and default initramfs
mkdir -p /tmp/alpine-iso
cd /tmp/alpine-iso
7z x /tmp/alpine-virt.iso boot/vmlinuz-virt boot/initramfs-virt 2>/dev/null \
  || isoinfo -i /tmp/alpine-virt.iso -x '/BOOT/VMLINUZ-VIRT;1' > boot/vmlinuz-virt
```

If `7z` and `isoinfo` aren't available:

```bash
apt-get install -y p7zip-full  # for 7z
# OR
mkdir /tmp/iso-mount && mount -o loop /tmp/alpine-virt.iso /tmp/iso-mount
cp /tmp/iso-mount/boot/vmlinuz-virt /tmp/alpine-iso/boot/
umount /tmp/iso-mount
```

### Building an initramfs / rootfs

The fastest way to get a usable root filesystem:

```bash
# Pull Alpine rootfs via Docker
docker pull alpine:3.21
container_id=$(docker create alpine:3.21)
mkdir -p /tmp/vm-rootfs
docker export "$container_id" | tar -xf - -C /tmp/vm-rootfs
docker rm "$container_id"
```

If Docker isn't working (it often has issues in gVisor), download the rootfs
tarball directly:

```bash
curl -L -o /tmp/alpine-minirootfs.tar.gz \
  https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/alpine-minirootfs-3.21.3-x86_64.tar.gz
mkdir -p /tmp/vm-rootfs
tar xzf /tmp/alpine-minirootfs.tar.gz -C /tmp/vm-rootfs
```

#### Creating the init script

Write an `init.sh` at the root of your rootfs:

```bash
cat > /tmp/vm-rootfs/init.sh << 'INITEOF'
#!/bin/sh
/bin/mount -t proc proc /proc 2>/dev/null
/bin/mount -t sysfs sys /sys 2>/dev/null
/bin/mount -t devtmpfs dev /dev 2>/dev/null

# IMPORTANT: redirect output to serial console, otherwise you won't see anything
exec >/dev/ttyS0 2>&1

echo "=== VM booted ==="
echo "Kernel: $(uname -a)"

# --- your commands here ---

# Power off when done (with -no-reboot, this exits QEMU)
exec /sbin/poweroff -f
INITEOF
chmod +x /tmp/vm-rootfs/init.sh
```

**Critical detail**: `exec >/dev/ttyS0 2>&1` is essential. Without it, your
script's output goes to `/dev/console` which is NOT the serial port, so you
see nothing in `-nographic` mode.

#### Packing the initramfs

```bash
apt-get install -y cpio  # if not already installed

cd /tmp/vm-rootfs
find . | cpio -o -H newc 2>/dev/null | gzip > /tmp/my-initramfs.cpio.gz
```

**Warning**: Do NOT try to create cpio archives with shell tricks or `tar`.
Use actual `cpio`. Previous attempts with fake cpio substitutes produced
43-byte files that silently failed.

### Booting QEMU

```bash
timeout 120 qemu-system-x86_64 \
  -kernel /tmp/alpine-iso/boot/vmlinuz-virt \
  -initrd /tmp/my-initramfs.cpio.gz \
  -append "console=ttyS0 rdinit=/init.sh loglevel=1" \
  -m 2048 -nographic -no-reboot \
  > /tmp/qemu-output.txt 2>/dev/null

cat /tmp/qemu-output.txt
```

Key flags explained:
- `-kernel` — use an external kernel instead of booting from disk
- `-initrd` — the cpio.gz initramfs that becomes the root filesystem
- `-append "console=ttyS0 rdinit=/init.sh loglevel=1"` — kernel command line:
  - `console=ttyS0` — send kernel messages to serial port
  - `rdinit=/init.sh` — run your script as PID 1 instead of `/init` or `/sbin/init`
  - `loglevel=1` — suppress most kernel messages (use `loglevel=7` for debug)
- `-m 2048` — 2GB RAM for the VM (adjust as needed)
- `-nographic` — no GUI, use serial console
- `-no-reboot` — exit QEMU when the guest powers off (instead of rebooting)

### Things to watch out for

**Capturing output**: With `-nographic`, QEMU sends serial output to stdout
and its own messages to stderr. Redirect stderr to `/dev/null` and capture
stdout to get clean guest output. Using `tee` sometimes loses output due to
buffering; redirect to a file instead.

**`-machine q35` conflicts**: If you get `drive with bus=0, unit=0 (index=0)
exists`, remove `-machine q35` and use the default (`pc`). This happens when
default IDE devices conflict.

**`-cpu max` crashes**: On gVisor, `-cpu max` causes assertion failures in
QEMU. Just use the default CPU (`qemu64`), which emulates an AMD-like
processor.

**`quiet` kernel flag**: Don't use it. It suppresses your init script output
too, not just kernel messages. Use `loglevel=1` instead to keep kernel
messages quiet while letting your script output through.

**Timeout**: Always use `timeout` around QEMU in case something hangs. 120
seconds is usually enough for simple tests. For the timeout to work, QEMU
must exit on its own (`-no-reboot` + `poweroff -f` in the init script).

## Getting KVM inside QEMU

This is the big trick. The QEMU `qemu64` CPU emulates an AMD-like processor.
The Linux `kvm_amd` kernel module can load on it with nested virtualization:

```bash
# In your init.sh, after mounting filesystems:
MODDIR=/lib/modules/$(uname -r)/kernel/arch/x86/kvm
insmod $MODDIR/kvm.ko
insmod $MODDIR/kvm-amd.ko    # This one actually works!
# insmod $MODDIR/kvm-intel.ko  # This FAILS: "VMX not supported by CPU 0"

if [ -e /dev/kvm ]; then
    echo "KVM available!"
    chmod 666 /dev/kvm
fi
```

The KVM modules are NOT included in Alpine's default initramfs. You need to
install them into your rootfs:

```bash
# On the host, get the modules from the Alpine kernel package
# The module path must match the kernel version you're booting
KVER=6.12.13-0-virt  # match your vmlinuz-virt version
mkdir -p /tmp/vm-rootfs/lib/modules/$KVER/kernel/arch/x86/kvm

# If you have apk (Alpine package manager) in the rootfs:
chroot /tmp/vm-rootfs apk add linux-virt  # gets the modules

# Or copy them from the ISO/package manually:
# The .ko files you need: kvm.ko, kvm-amd.ko
```

**Performance note**: KVM inside QEMU TCG is extremely slow (emulating
hardware virt in software). Simple commands work but don't expect to run
heavy workloads. A single `echo hello` in a libkrun microVM takes ~5 seconds
to boot and complete.

## Copying glibc binaries into Alpine

Alpine uses musl libc, not glibc. If you need to run glibc-linked binaries
(like Go programs built on the host), copy the glibc runtime:

```bash
# Copy glibc dynamic linker and core libs into the rootfs
mkdir -p /tmp/vm-rootfs/lib64 /tmp/vm-rootfs/usr/lib
cp /lib64/ld-linux-x86-64.so.2 /tmp/vm-rootfs/lib64/
cp /lib/x86_64-linux-gnu/libc.so.6 /tmp/vm-rootfs/usr/lib/
cp /lib/x86_64-linux-gnu/libpthread.so.0 /tmp/vm-rootfs/usr/lib/ 2>/dev/null
cp /lib/x86_64-linux-gnu/libdl.so.2 /tmp/vm-rootfs/usr/lib/ 2>/dev/null
cp /lib/x86_64-linux-gnu/libm.so.6 /tmp/vm-rootfs/usr/lib/ 2>/dev/null
cp /lib/x86_64-linux-gnu/libgcc_s.so.1 /tmp/vm-rootfs/usr/lib/ 2>/dev/null

# Set LD_LIBRARY_PATH in your init.sh
export LD_LIBRARY_PATH=/usr/lib
```

## What you can use QEMU for

### 1. Testing anything that needs KVM
As demonstrated with krunsh/libkrun. Any tool that requires `/dev/kvm` can
work inside QEMU TCG with the kvm_amd module loaded.

### 2. Testing kernel features not available in gVisor
gVisor has limited syscall support. If you need specific Linux features
(cgroups v2, namespaces, eBPF, specific filesystems, kernel modules), boot
a real kernel in QEMU.

### 3. Running a full Linux distribution
If you need a complete Linux environment with systemd, networking, package
management, etc., you can boot a full disk image in QEMU.

### 4. Testing kernel modules
If you're developing or testing kernel modules, QEMU gives you a real
kernel to load them into (unlike gVisor which has no module support at all).

### 5. Network testing
QEMU can set up virtual networks between VMs or between the VM and host
using `-netdev` and `-device` options. Useful for testing network services
in a real kernel environment.

### 6. Filesystem testing
Need to test ext4, btrfs, xfs, or other filesystems? QEMU boots a real
kernel that supports them all. Create a virtual disk with `-drive` and
format it inside the VM.

## Docker in gVisor: what works and what doesn't

Docker CAN start in gVisor with workarounds:

```bash
dockerd --iptables=false --bridge=none --storage-driver=vfs &
```

- `--iptables=false` — gVisor doesn't fully support netfilter
- `--bridge=none` — skip bridge network setup
- `--storage-driver=vfs` — overlay2 doesn't work under gVisor

Containers will run but they're still under gVisor (same kernel 4.4.0).
Docker is useful for pulling images and exporting rootfs tarballs, but
don't expect it to provide real kernel features. For that, use QEMU.

## Quick reference: minimal QEMU test

```bash
# 1. Get a kernel
apt-get install -y qemu-system-x86_64 cpio
curl -L -o /tmp/alpine-virt.iso \
  https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/alpine-virt-3.21.3-x86_64.iso
mkdir -p /tmp/alpine-iso/boot && cd /tmp/alpine-iso
7z x /tmp/alpine-virt.iso boot/vmlinuz-virt 2>/dev/null

# 2. Build a rootfs
curl -L https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/alpine-minirootfs-3.21.3-x86_64.tar.gz \
  | tar xz -C /tmp/vm-rootfs

# 3. Write init script
cat > /tmp/vm-rootfs/init.sh << 'EOF'
#!/bin/sh
mount -t proc proc /proc; mount -t sysfs sys /sys; mount -t devtmpfs dev /dev
exec >/dev/ttyS0 2>&1
echo "Hello from real Linux $(uname -r)!"
exec poweroff -f
EOF
chmod +x /tmp/vm-rootfs/init.sh

# 4. Pack and boot
cd /tmp/vm-rootfs && find . | cpio -o -H newc 2>/dev/null | gzip > /tmp/initramfs.cpio.gz
timeout 60 qemu-system-x86_64 \
  -kernel /tmp/alpine-iso/boot/vmlinuz-virt \
  -initrd /tmp/initramfs.cpio.gz \
  -append "console=ttyS0 rdinit=/init.sh loglevel=1" \
  -m 512 -nographic -no-reboot 2>/dev/null
```

Expected output includes: `Hello from real Linux 6.12.13-0-virt!`
