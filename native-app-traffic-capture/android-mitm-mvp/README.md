# Android MITM MVP

Single-container environment to demonstrate certificate pinning bypass and HTTPS traffic capture for Android apps using Frida and mitmproxy.

## Components
- Android emulator with noVNC access (from `budtmo/docker-android:emulator_13.0`)
- mitmproxy with web UI on port `8081`
- Frida server plus HTTP Toolkit interception scripts
- Entry script automating proxy configuration, CA trust injection, and Frida app launch

## Prerequisites
1. Docker 28.x or later installed locally (`docker --version`).
2. Host with ≥8 GB RAM; 16 GB recommended for smoother emulator performance.
3. macOS or Linux host (tested on macOS 14 / Apple Silicon via emulation).

## Repository Layout
```
android-mitm-mvp/
├── Dockerfile
├── entrypoint.sh
├── frida-scripts/
│   ├── config.js
│   ├── native-connect-hook.js
│   ├── native-tls-hook.js
│   └── android/
│       ├── android-proxy-override.js
│       ├── android-system-certificate-injection.js
│       ├── android-certificate-unpinning.js
│       ├── android-certificate-unpinning-fallback.js
│       └── android-disable-root-detection.js
└── README.md
```

## Build
```
cd native-app-traffic-capture/android-mitm-mvp
docker build -t android-mitm-mvp .
```

## Run

### Local Development
```
docker run -it --rm \
  --privileged \
  -p 6080:6080 \
  -p 8081:8081 \
  -e APP_PACKAGE=com.android.chrome \
  -e DEVICE="Samsung Galaxy S23" \
  android-mitm-mvp
```

### GCP/Cloud Deployment
For optimal performance on cloud VMs with nested virtualization support:
```
docker run -d \
  --name android-mitm-mvp \
  --privileged \
  --device /dev/kvm \
  -e WEB_VNC=true \
  -e APP_PACKAGE=com.android.chrome \
  -e EMULATOR_ADDITIONAL_ARGS='-cores 4 -memory 8192' \
  -e EMULATOR_DATA_PARTITION='2048m' \
  -p 0.0.0.0:6080:6080 \
  -p 0.0.0.0:8081:8081 \
  android-mitm-mvp:latest
```

**Note**: The `--device /dev/kvm` flag enables hardware acceleration on Linux hosts. The entrypoint gracefully handles missing KVM by falling back to software acceleration.

### Environment Overrides
- `APP_PACKAGE`: Android package to spawn with Frida (default `com.android.chrome`).
- `DEVICE`: Emulator profile name understood by the base image supervisor (default `Samsung Galaxy S23`).
- `EMULATOR_ADDITIONAL_ARGS`: Extra flags passed to the Android emulator. Defaults to `-cores 4 -memory 8192 -no-snapshot -no-boot-anim -noaudio -gpu swiftshader_indirect` for faster boot and reduced resource usage on nested virtualization hosts.
- `ANDROID_PROXY_HOST` / `ANDROID_PROXY_PORT`: Proxy host/port pushed into the emulator settings (default `10.0.2.2:8080`). The entrypoint re-applies these values during verification to avoid the base image resetting them to `127.0.0.1:8080`.
- `SKIP_FRIDA`: Set to `true` to skip loading Frida (useful for debugging proxy/cert only flows).

## Verification Checklist
1. Wait for `🎉 Setup complete!` message in container logs.
2. Check the traffic validation output (step 8) confirms:
   - App is running
   - Proxy is configured
   - mitmproxy web UI is accessible
3. Open `http://localhost:6080` to control the emulator via noVNC.
4. Open `http://localhost:8081` to inspect mitmproxy traffic.
5. If using Chrome (default), a test navigation to `https://www.google.com` is automatically triggered - check mitmproxy UI for captured HTTPS traffic.
6. In noVNC, navigate to other sites or trigger additional network requests.
7. Confirm decrypted requests/responses appear within the mitmproxy UI.

## Logs & Monitoring
- mitmproxy web & proxy: `/var/log/mitmproxy.log`
- Frida server stdout: `/var/log/frida-server.log`
- Frida app session: `/var/log/frida-app.log`
- View combined logs live within the container (`tail -F`).

## Troubleshooting

### Common Issues

- **Frida not running**: `docker exec -it <container> cat /var/log/frida-server.log`
- **App spawn issues**: `docker exec -it <container> cat /var/log/frida-app.log`
- **No traffic captured**:
  - `docker exec -it <container> adb shell settings get global http_proxy`
  - `docker exec -it <container> curl http://localhost:8081`
- **Certificate trust errors**:
  - `docker exec -it <container> adb shell ls /system/etc/security/cacerts/ | grep '.0'`
  - Inspect `frida-app.log` for injection messages
- **Emulator exits with `/dev/kvm` or Rosetta errors**: the stock `budtmo/docker-android` image expects hardware virtualization. Run the container on a Linux host with `/dev/kvm` available, or swap in an arm64-friendly base image to avoid Rosetta emulation limits on Apple Silicon.

### Boot Issues

- **Device boot hangs beyond 300s**:
  - Check `/var/log/mitmproxy.log` inside container
  - Verify emulator CPU usage (should be > 50% during boot)
  - Check noVNC display (port 6080) for visual feedback
  - May need to increase timeout or verify nested virtualization support on host
- **Boot animation stuck**: The entrypoint injects wake events at 30s and 45s to help progression. If still stuck, check emulator logs via `docker logs <container>`.

### Port & Service Issues

- **Ports become unresponsive**:
  - Verify container running: `docker ps | grep android-mitm-mvp`
  - Check process: `docker top android-mitm-mvp | grep mitmproxy`
  - Review logs: `docker logs android-mitm-mvp`
- **mitmproxy Web UI returns 403**: Expected without proper HTTP headers. Use a browser (which sends proper headers) instead of curl.

### Build Issues

- **Platform mismatch errors** (Apple Silicon): Use `docker build --platform linux/amd64` to force x86_64 build
- **Permission denied during build**: Base image requires `USER root` for package installation steps
- **PEP 668 managed environment**: Dockerfile sets `PIP_BREAK_SYSTEM_PACKAGES=1` to allow pip installs

## Cloud Deployment (GCP)
The `scripts/` directory contains helper scripts for Google Cloud:

- `scripts/start_vm.sh` provisions a nested-virtualization-compatible VM (`n2-standard-4` by default for extra CPU/RAM) in the `corsali-development` project, opens ports `6080`/`8081`, uploads this project, builds the Docker image, and launches the container. Customize via environment variables:
  - `PROJECT`, `ZONE`, `INSTANCE_NAME`, `MACHINE_TYPE`, `BOOT_DISK_SIZE`, `NETWORK_TAG`, `FIREWALL_RULE`.
  - Android emulator tuning knobs via env: `APP_PACKAGE` (defaults to `com.android.chrome`), `EMULATOR_ADDITIONAL_ARGS` (defaults to `-cores 4 -memory 8192`), and `EMULATOR_DATA_PARTITION` (defaults to `2048m`).
  - The script auto-generates an SSH key at `~/.ssh/android-mitm-mvp` (configurable via `SSH_KEY_FILE`) and injects it into instance metadata so subsequent `gcloud compute ssh/scp` calls work without OS Login.
- noVNC is exposed automatically (`WEB_VNC=true`), so once the script prints the public IP you can browse to `http://<ip>:6080` and click **Connect** to control the emulator.
- `scripts/stop_vm.sh` deletes the instance and optionally the firewall rule (`DELETE_FIREWALL=true`).

Requirements: authenticated `gcloud` CLI with project access. The scripts print the public IP for noVNC and mitmproxy after deployment.

### Deployment Status & Access Points
**Last Successful Deployment**: 2025-11-14 17:00 UTC

**Access Points** (when deployed):
- **noVNC**: `http://<vm-ip>:6080/` - Web-based VNC client for Android emulator control
- **mitmproxy Web UI**: `http://<vm-ip>:8081/` - Traffic inspection interface

**Ports Exposed**:
- `6080` - noVNC web interface
- `8081` - mitmproxy web UI
- `8080` - mitmproxy proxy listener
- `5554-5555` - ADB (emulator console/daemon)
- `5900` - VNC raw protocol
- `4723` - Appium server
- `9000` - Web log viewer

### Boot Timeline
Typical startup sequence on GCP (n2-standard-4 VM with nested virtualization):
- **0s**: mitmproxy initialization completes (< 1s)
- **0-1s**: Android emulator launch begins
- **0s**: ADB device detection (immediate)
- **15s**: Boot animation active (`bootanim=running`)
- **30s**: Wake event injection (helps boot progression)
- **~120s**: Device online (`device` state in `adb devices`)
- **120-180s**: Full boot completion (`boot_completed=1`)

**Resource Usage** (during boot):
- Memory: ~4GB / 15.6GB (26%)
- CPU: ~150% (multi-core, normal during boot)
- Disk I/O: Active during boot sequence

### Key Fixes & Improvements

**KVM Handling** (2025-11-14):
- Entrypoint patches `emulator.py` to replace `-accel on` with `-accel off` (disables hardware acceleration)
- Changes RuntimeError to warning if `/dev/kvm` is missing, allowing graceful degradation
- Container must be launched with `--device /dev/kvm` for optimal performance on GCP

**Boot Optimization**:
- Reduced boot timeout from 240s to 90s (further optimized to 300s for reliability)
- Wake event injection at 30s and 45s to help boot progression
- Enhanced setup wizard dismissal with multi-strategy approach (back button, tap "Skip"/"Next", home button, swipe gestures)
- Automated unlock/home navigation so emulator lands on launcher after boot

**Traffic Validation**:
- Added automated traffic validation step that verifies:
  - App is running (via `pidof`)
  - Proxy configuration is set correctly
  - mitmproxy web UI is accessible
- For Chrome: automatically triggers test navigation to `https://www.google.com` to generate initial traffic
- Improved completion message with traffic capture status summary

**Default Configuration**:
- Changed default app from `com.whatsapp` to `com.android.chrome` for easier traffic validation
- Default emulator args: `-cores 4 -memory 8192` with larger data partition (2048m)
- Default machine type for GCP: `n2-standard-4` for better CPU/RAM allocation

## Extending the MVP
- Pre-bundle APKs by copying them into the image and installing during build.
- Customize `APP_PACKAGE` at runtime to test other targets (e.g., Instagram or banking apps).
- Export mitmproxy flows (`mitmdump --save-stream-file`) for archival or downstream analysis.
- Integrate automated capture scripts once the proof of concept is confirmed.

## Attribution
Frida interception scripts sourced from [HTTP Toolkit](https://github.com/httptoolkit/frida-interception-and-unpinning) under AGPL-3.0-or-later.
