env86 is a Go-based management tool that enables users to run x86 Linux virtual machines within browser contexts via the v86 WebAssembly emulator. By combining a native desktop application (embedding a browser), a robust CLI, and an integrated virtual networking stack, env86 provides an easily distributable and reproducible Linux environment that can boot instantly from snapshots, support host-guest communication, and mount host filesystems. Images are efficiently distributed through GitHub releases, and the system can be used interactively or in headless/automation contexts, making it especially suitable for development, education, sandboxing, legacy software execution, and rapid demonstration scenarios. While performance is limited by browser-based emulation, env86 uniquely excels in cross-platform portability and accessibility, allowing VMs to run anywhere a browser or desktop is available.

Key findings/features:
- Instant VM boot from state snapshots, dramatically reducing startup time.
- Secure host-guest communication using CBOR RPC over serial ports.
- User-space networking via [go-netstack](https://github.com/progrium/go-netstack) and transparent port forwarding.
- 9P protocol-based filesystem mounting for host directory access.
- Image distribution and updates handled via GitHub releases, supporting reproducible environments.
- Desktop integration via [tractor.dev/toolkit-go](https://github.com/tractor-dev/toolkit-go) enables GUI and CLI/CI workflows.

See the env86 repo for details: https://github.com/progrium/env86  
Learn more about the v86 emulator: https://github.com/copy/v86
