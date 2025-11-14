# Notes - Android MITM MVP

## 2025-11-13
- Initialized project workspace at `native-app-traffic-capture/android-mitm-mvp`.
- Confirmed Docker available: `Docker version 28.5.2, build ecc6942`.
- Goal: reproduce MVP flow from `native-app-traffic-capture/mvp-guide.md` inside single container.

### Next Steps
- Import HTTP Toolkit Frida scripts into `frida-scripts/`.
- Draft Dockerfile and entrypoint using guide as baseline, capture any deviations.

## Frida Script Import
- Pulled HTTP Toolkit scripts via `git clone --depth 1 https://github.com/httptoolkit/frida-interception-and-unpinning.git`.
- Copied `config.js`, `native-connect-hook.js`, `native-tls-hook.js`, and `android/` directory into `frida-scripts/`.
- Removed temporary clone to keep workspace minimal.

## Container Build Assets
- Authored `Dockerfile` based on `budtmo/docker-android:emulator_13.0` with python/frida dependencies and mitmproxy priming.
- Created `entrypoint.sh` orchestrating mitmproxy, emulator boot, certificate install, proxy config, Frida startup, and app launch.
- Automated `config.js` customization via Python regex to preserve upstream logic while injecting the runtime certificate and proxy settings.
- Added HTTP proxy host/port duplication in global settings to improve reliability on recent Android builds.

## Documentation & Validation Plan
- Authored `README.md` with build/run instructions, environment overrides, verification checklist, troubleshooting paths, and extension ideas.
- Verification flow mirrors MVP guide: use noVNC (`:6080`) for manual interaction and mitmproxy web (`:8081`) to confirm decrypted traffic.
- Pending manual test once container build completes; expect mitmproxy flows for WhatsApp registration/login requests as success criteria.

## Build Attempt 2025-11-13
- `docker build -t android-mitm-mvp .` failed on base image extraction.
- Error: `failed to register layer: write /opt/android/system-images/.../system.img: input/output error`.
- Docker warning indicated platform mismatch: host defaulting to `linux/arm64` while base image is `linux/amd64`.
- Suspect resolution: force buildx to use `--platform linux/amd64` or enable emulation; alternatively find arm64-compatible base image.

## Build Attempt 2025-11-13 (platform override)
- Command: `docker build --platform linux/amd64 -t android-mitm-mvp .`
- Result: `failed to solve: Internal: write /var/lib/docker/buildkit/containerdmeta.db: input/output error`.
- Indicates local BuildKit metadata store corruption or insufficient disk space on Docker Desktop.
- Next actions to try:
  1. Restart Docker Desktop to reset BuildKit state.
  2. Run `docker system prune --volumes` (after backup) to clear build cache.
  3. Verify disk availability and Docker data-root permissions.

## Build Troubleshooting 2025-11-13 (permission)
- Attempted to create `/var/lib/apt/lists/partial` during build; `mkdir` failed with permission denied.
- Indicates base image default user lacks privileges. Need to escalate to root for package steps.
- Plan: `USER root` prior to `apt-get` layer, perform installs, then `USER android` (original user) before runtime steps if required.

## Dockerfile Update
- Added explicit `USER root` before package installation and reverted to `USER android` post-setup to satisfy base image permission constraints.
- Maintains upstream default runtime user while still allowing apt/pip layers to succeed.
- Addressed PEP 668 managed environment by setting `PIP_BREAK_SYSTEM_PACKAGES=1` during pip install in Dockerfile.
- Docker build succeeded with `--platform linux/amd64` after pip override; image tagged `android-mitm-mvp`.
- Container runtime patch automatically relaxes KVM check and forces software acceleration by rewriting emulator.py during startup.
- On Apple Silicon host, emulator launches but fails under Rosetta (`rosetta error: Unimplemented syscall number 282`); `adb wait-for-device` stalls.
- Full proof-of-concept likely requires Linux host with /dev/kvm or alternate arm64 base image; document limitation in README troubleshooting.
- Switched base image to `budtmo/docker-android:emulator_arm64_13.0` and download Frida arm64 server to support Apple Silicon hosts.
- Attempted to switch to `budtmo/docker-android:emulator_arm64_13.0`, but Docker Hub has no arm64 tags; reverted to x86_64 base.
- Added `scripts/start_vm.sh` and `scripts/stop_vm.sh` to provision an n2-standard-2 GCE VM with nested virtualization, install Docker, deploy the container, and tear it down in project `corsali-develipment`.
- Corrected project spelling to `corsali-development` across GCP deployment scripts.
- Verified `scripts/start_vm.sh` end-to-end: provisioned `n2-standard-2` VM in `corsali-development`, built image, and confirmed mitmproxy reachable (HTTP 405 on HEAD) and emulator present as `emulator-5554` after boot. Captured public IP `34.42.16.156` during test.
- Used `DELETE_FIREWALL=true ./scripts/stop_vm.sh` to tear down the VM and associated firewall rule after testing.
- Updated GCP start script to manage SSH keys explicitly and enable noVNC by setting `WEB_VNC=true`.
- Re-ran deployment; verified noVNC via browser (Playwright screenshot captured) and mitmproxy reachability on `34.133.204.197`.
- Noted emulator requires ~2 minutes post-launch before `adb devices` reports `device` state.
- Increased default GCE machine type to `n2-standard-4` and default emulator args (`-cores 4 -memory 8192`, larger data partition) with Chrome as target app.
- Added automated unlock/home navigation in entrypoint so the emulator lands on the launcher after boot.

## Boot Optimization & Traffic Validation 2025-01-XX
- **Reduced boot timeout**: Changed from 240s (4 min) to 90s to short-circuit boot wait and proceed faster to proxy/Frida setup.
- **Early key event injection**: Added wake/unlock/home events at 30s and 45s during boot loop to help progression.
- **Enhanced setup wizard dismissal**: Added multi-strategy approach:
  - Back button presses
  - Tap "Skip" button (bottom right: 700, 2000)
  - Tap "Next" button (bottom center: 500, 1800)
  - Multiple home button presses
  - Swipe up gesture to dismiss overlays
- **Changed default app**: Updated `APP_PACKAGE` default from `com.whatsapp` to `com.android.chrome` for easier traffic validation.
- **Added traffic validation step**: New step 8 verifies:
  - App is running (via `pidof`)
  - Proxy configuration is set correctly
  - mitmproxy web UI is accessible
  - For Chrome: automatically triggers test navigation to `https://www.google.com` to generate initial traffic
- **Improved completion message**: Added traffic capture status summary and clearer instructions for viewing captured traffic.
