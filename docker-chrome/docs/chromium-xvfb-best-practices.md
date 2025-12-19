# Research Summary: Chromium in Containers with Xvfb

## Overview
This research focused on identifying best practices for running non-headless Chromium in containerized environments using Xvfb. It addresses common stability issues, performance pitfalls, and configuration requirements for production deployments.

## Key Takeaways
- **Flag Configuration**:
    - `--no-sandbox`: Essential for most container environments to avoid kernel-level isolation issues.
    - `--disable-dev-shm-usage`: **Critical** for preventing crashes when the Docker `/dev/shm` partition is limited (default 64MB). It forces Chromium to use `/tmp` for shared memory.
    - `--disable-gpu`: Recommended for pure software rendering to simplify the stack if hardware acceleration is not required.
- **Memory Management**:
    - **Shared Memory**: If high performance is needed, increase the Docker `--shm-size` (e.g., 1GB-2GB) instead of disabling it.
    - **Leaks**: Chromium tends to leak memory in long-running sessions; periodic restarts are recommended.
    - **Zombies**: Use an init system like `tini` to prevent zombie processes.
- **Display & Resizing**:
    - **Xvfb Limitation**: Standard Xvfb cannot dynamically resize its framebuffer.
    - **Dynamic Resize**: Use **xpra** or **TigerVNC** with RandR support if resolution changes are required without restarting the X server.
- **GPU & WebGL**:
    - WebGL requires a display server (Xvfb) and refuses to run in true `--headless` mode.
    - Requires Mesa libraries (`libgl1-mesa-glx`, `libgl1-mesa-dri`) for hardware or high-quality software acceleration (SwiftShader).

## Stability Checkpoints
1. **Startup Synchronization**: Wait for the X11 socket (e.g., `/tmp/.X11-unix/X99`) to be present before launching Chromium.
2. **Process Management**: Use `supervisord` or `systemd` to ensure both Xvfb and Chromium are restarted upon failure.
3. **Resource Isolation**: Use `cgroups` or Docker limits to prevent Chromium from exhausting host resources.

## Example Configuration (Minimal)
```bash
Xvfb :99 -screen 0 1920x1080x24 -ac +extension GLX +render -noreset &
export DISPLAY=:99
chromium-browser --no-sandbox --disable-dev-shm-usage --disable-gpu
```

## Conclusion
Running Chromium in Xvfb is mature but requires precise configuration of shared memory, sandboxing flags, and process management to achieve production-grade stability.
