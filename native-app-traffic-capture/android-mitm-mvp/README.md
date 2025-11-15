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
```
docker run -it --rm \
  --privileged \
  -p 6080:6080 \
  -p 8081:8081 \
  -e APP_PACKAGE=com.android.chrome \
  -e DEVICE="Samsung Galaxy S23" \
  android-mitm-mvp
```

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
- **Frida not running**: `docker exec -it <container> cat /var/log/frida-server.log`
- **App spawn issues**: `docker exec -it <container> cat /var/log/frida-app.log`
- **No traffic captured**:
  - `docker exec -it <container> adb shell settings get global http_proxy`
  - `docker exec -it <container> curl http://localhost:8081`
- **Certificate trust errors**:
  - `docker exec -it <container> adb shell ls /system/etc/security/cacerts/ | grep '.0'`
  - Inspect `frida-app.log` for injection messages
- **Emulator exits with `/dev/kvm` or Rosetta errors**: the stock `budtmo/docker-android` image expects hardware virtualization. Run the container on a Linux host with `/dev/kvm` available, or swap in an arm64-friendly base image to avoid Rosetta emulation limits on Apple Silicon.

## Cloud Deployment (GCP)
The `scripts/` directory contains helper scripts for Google Cloud:

- `scripts/start_vm.sh` provisions a nested-virtualization-compatible VM (`n2-standard-4` by default for extra CPU/RAM) in the `corsali-development` project, opens ports `6080`/`8081`, uploads this project, builds the Docker image, and launches the container. Customize via environment variables:
  - `PROJECT`, `ZONE`, `INSTANCE_NAME`, `MACHINE_TYPE`, `BOOT_DISK_SIZE`, `NETWORK_TAG`, `FIREWALL_RULE`.
  - Android emulator tuning knobs via env: `APP_PACKAGE` (defaults to `com.android.chrome`), `EMULATOR_ADDITIONAL_ARGS` (defaults to `-cores 4 -memory 8192`), and `EMULATOR_DATA_PARTITION` (defaults to `2048m`).
  - The script auto-generates an SSH key at `~/.ssh/android-mitm-mvp` (configurable via `SSH_KEY_FILE`) and injects it into instance metadata so subsequent `gcloud compute ssh/scp` calls work without OS Login.
- noVNC is exposed automatically (`WEB_VNC=true`), so once the script prints the public IP you can browse to `http://<ip>:6080` and click **Connect** to control the emulator.
- `scripts/stop_vm.sh` deletes the instance and optionally the firewall rule (`DELETE_FIREWALL=true`).

Requirements: authenticated `gcloud` CLI with project access. The scripts print the public IP for noVNC and mitmproxy after deployment.

## Extending the MVP
- Pre-bundle APKs by copying them into the image and installing during build.
- Customize `APP_PACKAGE` at runtime to test other targets (e.g., Instagram or banking apps).
- Export mitmproxy flows (`mitmdump --save-stream-file`) for archival or downstream analysis.
- Integrate automated capture scripts once the proof of concept is confirmed.

## Attribution
Frida interception scripts sourced from [HTTP Toolkit](https://github.com/httptoolkit/frida-interception-and-unpinning) under AGPL-3.0-or-later.
