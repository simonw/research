# Research projects carried out by AI tools

Each directory in this repo is a separate research project carried out by an LLM tool - usually [Claude Code](https://www.claude.com/product/claude-code). Every single line of text and code was written by an LLM.

<!--[[[cog
import os
import subprocess
import pathlib
from datetime import datetime, timezone

# Model to use for generating summaries
MODEL = "gemini-3-flash-preview"

# Get all subdirectories with their first commit dates
research_dir = pathlib.Path.cwd()
subdirs_with_dates = []

for d in research_dir.iterdir():
    if d.is_dir() and not d.name.startswith('.'):
        # Get the date of the first commit that touched this directory
        try:
            result = subprocess.run(
                ['git', 'log', '--diff-filter=A', '--follow', '--format=%aI', '--reverse', '--', d.name],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0 and result.stdout.strip():
                # Parse first line (oldest commit)
                date_str = result.stdout.strip().split('\n')[0]
                commit_date = datetime.fromisoformat(date_str.replace('Z', '+00:00'))
                subdirs_with_dates.append((d.name, commit_date))
            else:
                # No git history, use directory modification time
                subdirs_with_dates.append((d.name, datetime.fromtimestamp(d.stat().st_mtime, tz=timezone.utc)))
        except Exception:
            # Fallback to directory modification time
            subdirs_with_dates.append((d.name, datetime.fromtimestamp(d.stat().st_mtime, tz=timezone.utc)))

# Print the heading with count
print(f"## {len(subdirs_with_dates)} research projects\n")

# Sort by date, most recent first
subdirs_with_dates.sort(key=lambda x: x[1], reverse=True)

for dirname, commit_date in subdirs_with_dates:
    folder_path = research_dir / dirname
    readme_path = folder_path / "README.md"
    summary_path = folder_path / "_summary.md"

    date_formatted = commit_date.strftime('%Y-%m-%d')

    # Get GitHub repo URL
    github_url = None
    try:
        result = subprocess.run(
            ['git', 'remote', 'get-url', 'origin'],
            capture_output=True,
            text=True,
            timeout=2
        )
        if result.returncode == 0 and result.stdout.strip():
            origin = result.stdout.strip()
            # Convert SSH URL to HTTPS URL for GitHub
            if origin.startswith('git@github.com:'):
                origin = origin.replace('git@github.com:', 'https://github.com/')
            if origin.endswith('.git'):
                origin = origin[:-4]
            github_url = f"{origin}/tree/main/{dirname}"
    except Exception:
        pass

    if github_url:
        print(f"### [{dirname}]({github_url}) ({date_formatted})\n")
    else:
        print(f"### {dirname} ({date_formatted})\n")

    # Check if summary already exists
    if summary_path.exists():
        # Use cached summary
        with open(summary_path, 'r') as f:
            description = f.read().strip()
            if description:
                print(description)
            else:
                print("*No description available.*")
    elif readme_path.exists():
        # Generate new summary using llm command
        prompt = """Summarize this research project concisely. Write just 1 paragraph (3-5 sentences) followed by an optional short bullet list if there are key findings. Vary your opening - don't start with "This report" or "This research". Include 1-2 links to key tools/projects. Be specific but brief. No emoji."""
        result = subprocess.run(
            ['llm', '-m', MODEL, '-s', prompt],
            stdin=open(readme_path),
            capture_output=True,
            text=True,
            timeout=60
        )
        if result.returncode != 0:
            error_msg = f"LLM command failed for {dirname} with return code {result.returncode}"
            if result.stderr:
                error_msg += f"\nStderr: {result.stderr}"
            raise RuntimeError(error_msg)
        if result.stdout.strip():
            description = result.stdout.strip()
            print(description)
            # Save to cache file
            with open(summary_path, 'w') as f:
                f.write(description + '\n')
        else:
            raise RuntimeError(f"LLM command returned no output for {dirname}")
    else:
        print("*No description available.*")

    print()  # Add blank line between entries

]]]-->
## 10 research projects

### [passkey-encryption-demo](https://github.com/Kahtaf/research/tree/main/passkey-encryption-demo) (2026-01-07)

Leveraged via a minimal Next.js application, this project explores how the WebAuthn PRF extension can generate stable, hardware-backed keys for client-side data protection. By integrating HKDF to derive AES-GCM encryption keys from passkey outputs, the demo enables secure local encryption of text and files without ever exposing secrets to a server. The implementation serves as a functional proof-of-concept for zero-knowledge architectures, highlighting the seamless transition from biometric authentication to cryptographic operations within the browser.

* Registers resident passkeys specifically utilizing the WebAuthn PRF extension.
* Derives stable AES-GCM keys for local file and text encryption via HKDF.
* Operates entirely client-side to ensure no sensitive key material reaches the backend.

Key resources: [WebAuthn PRF Extension](https://w3c.github.io/webauthn/#prf-extension) and [Next.js Framework](https://nextjs.org/)

### [cloudflare-remote-browser](https://github.com/Kahtaf/research/tree/main/cloudflare-remote-browser) (2025-12-23)

This architecture bridges the gap between fully autonomous agents and human oversight by hosting Playwright-driven browsers on Cloudflare’s edge infrastructure. It leverages Cloudflare Workers and Durable Objects to maintain stateful sessions, streaming live browser frames to a Next.js frontend via the Chrome DevTools Protocol (CDP). The system’s primary innovation is a "user takeover" mechanism that pauses automated execution to allow for manual intervention during 2FA, logins, or complex captchas. Integration with [Cloudflare Browser Rendering](https://developers.cloudflare.com/browser-rendering/) and [Playwright](https://playwright.dev/) provides a managed environment capable of handling high-latency automation tasks without local resource overhead.

**Key Features**
* Live, low-latency browser streaming using CDP screencast frames and WebSockets.
* Bi-directional input forwarding for remote mouse and keyboard control during takeover states.
* Sophisticated captcha detection that inspects internal library configurations across cross-origin iframes.
* Persistent session management through Durable Objects to prevent execution loss during human interaction.

### [docker-chrome-neko](https://github.com/Kahtaf/research/tree/main/docker-chrome-neko) (2025-12-19)

Docker Chrome Neko integrates high-performance WebRTC streaming with the Chrome DevTools Protocol to enable sophisticated remote browser manipulation. Users interact with a containerized Chromium instance through a Next.js control panel that supports real-time navigation, JavaScript injection, and network traffic monitoring. By leveraging the [neko](https://github.com/m1k1o/neko) framework for low-latency video delivery, the project ensures a responsive experience for complex automation and remote exploration tasks. The architecture combines an Express-based agent with persistent script capabilities to provide a robust environment for web testing and data capture via [CDP](https://chromedevtools.github.io/devtools-protocol/).

* WebRTC-powered streaming for low-latency remote interaction.
* Comprehensive API for network traffic capture and responsive viewport management.
* Support for both one-time and persistent JavaScript injection.
* Integrated Next.js dashboard for simplified session control.

### [docker-chrome](https://github.com/Kahtaf/research/tree/main/docker-chrome) (2025-12-18)

Docker Chrome Cloud provides a serverless-ready Chromium environment that enables remote browser automation and manual interaction via a low-latency WebRTC stream. Built on a containerized architecture using Playwright and the Chrome DevTools Protocol (CDP), the system facilitates stealthy web scraping with integrated bot-detection bypasses and full network traffic capture. Users can seamlessly switch between automated workflows and human intervention, making it ideal for tasks requiring complex logins or captcha resolution. This infrastructure is optimized for rapid deployment on Google Cloud Run or standalone virtual machines to support scalable, ephemeral browser sessions.

*   CDP-based stealth mode to remove browser fingerprints and bypass bot detection.
*   Decrypted HTTPS network capture for inspecting headers and request bodies.
*   Remote control capabilities through KasmVNC for real-time human interaction.
*   Stateless architecture designed for rapid deployment and on-demand session resets.

- [Playwright](https://playwright.dev/)
- [KasmVNC](https://github.com/kasmtech/KasmVNC)

### [redroid](https://github.com/Kahtaf/research/tree/main/redroid) (2025-12-15)

ReDroid Cloud automates the deployment of ephemeral Android emulators by leveraging serverless management and Google Compute Engine (GCE) infrastructure. The system allows users to provision high-performance Android 13 containers on demand, interact with them directly in a web browser, and terminate the instances to ensure resources are only paid for during active use. By combining Docker-based virtualization with WebSocket streaming, the project provides a scalable solution for testing and remote mobile access without the overhead of permanent hardware.

*   Ephemeral VM lifecycle management via a Next.js control panel and GCP SDK.
*   Low-latency visual streaming through ws-scrcpy and noVNC integrations.
*   Zero-cost idle state with an estimated operating cost of $0.02 per 15-minute session.
*   Support for GPU acceleration and nested virtualization on n1-standard-4 instances.

Links:
- ReDroid Project: https://github.com/remote-android/redroid-doc
- ws-scrcpy: https://github.com/NetEase-Game/ws-scrcpy

### [gcp-android-mitm-deployment](https://github.com/Kahtaf/research/tree/main/gcp-android-mitm-deployment) (2025-11-16)

Engineers successfully deployed the [dockerify-android-mitm](https://github.com/sh4hin/dockerify-android-mitm) solution on a GCP virtual machine to facilitate automated network traffic analysis. This environment runs a root-enabled Android 11 emulator within a Docker container, utilizing [mitmproxy](https://mitmproxy.org/) and iptables for seamless, system-wide HTTPS interception. The system provides immediate access to live traffic logs and device interaction via web-based dashboards for both proxy monitoring and real-time screen mirroring.

Key findings and features:
* Verified capture of over 32,000 network flows including decrypted TLSv1.3 traffic.
* Full device interaction enabled via ws-scrcpy on port 8000 and mitmproxy UI on port 8081.
* Automated redirection architecture ensures all application traffic is routed through the proxy without manual configuration.
* Stable deployment confirmed with healthy container status and complete Android boot verification.

### [zk-tls](https://github.com/Kahtaf/research/tree/main/zk-tls) (2025-11-14)

Moving beyond standard web-based attestation, this analysis proposes a hybrid architecture centered on self-hosted [TLSNotary](https://github.com/tlsnotary/tlsn) for its cryptographic flexibility and [Reclaim Protocol’s attestor-core](https://github.com/reclaimprotocol/attestor-core) for rapid mobile deployment. While existing solutions excel at static HTTPS request-response patterns, a significant technical gap remains regarding IoT protocols like MQTT and real-time WebSocket streaming. Developing custom protocol extensions on top of TLSNotary's MPC-based primitives provides the most viable path for long-term customization and sovereign data ingestion across diverse hardware and streaming sources.

**Key Findings:**
* No current zkTLS implementation supports persistent WebSocket connections or MQTT messaging.
* TLSNotary provides the modular Rust foundation necessary to build custom IoT and streaming extensions.
* Reclaim’s self-hosted infrastructure is the fastest route for scraping mobile WebView data like LinkedIn.
* Native app traffic capture requires an OS-level VPN/Proxy layer to bypass certificate pinning before attestation.

### [android-mitm-mvp-validation](https://github.com/Kahtaf/research/tree/main/android-mitm-mvp-validation) (2025-11-14)

Validation of the Android MITM MVP container on Google Cloud Platform demonstrates successful emulator deployment and proxy integration, though a critical failure in the Frida server currently blocks automated end-to-end testing. While the Android 13 environment boots successfully and allows for user-level certificate installation, the inability to initialize Frida prevents necessary application hooks in Chrome. Developers must now investigate potential architecture mismatches or SELinux restrictions that led to the Frida startup error to restore full interception capabilities. This status highlights a functional infrastructure that is one component away from operational readiness for traffic analysis.

*   Successful 84-second emulator boot and ADB connectivity.
*   Working [mitmproxy](https://mitmproxy.org/) web UI and proxy configuration.
*   [Frida](https://frida.re/) server startup failure (exit code 1) preventing E2E hooks.
*   System partition read-only errors on Android 13 necessitating user-store certificate fallbacks.

### [native-app-traffic-capture](https://github.com/Kahtaf/research/tree/main/native-app-traffic-capture) (2025-11-13)

Navigating the security restrictions of modern mobile operating systems requires a strategic mix of local VPN interception and cloud-based emulation to capture encrypted HTTPS traffic. While iOS remains relatively accessible for man-in-the-middle attacks via user-installed certificates, Android 7+ fundamentally blocks this approach for most apps by ignoring user-trusted authorities by default. The most robust capture methodology involves utilizing cloud-hosted emulators with system-level certificate access to bypass local device limitations without modifying the application binary. Key implementations for this research include [ProxyPin](https://github.com/wanghongenpin/network_proxy_flutter) for cross-platform local interception and [PCAPdroid](https://github.com/emanuele-f/PCAPdroid) for non-invasive Android network analysis.

**Key Findings**

* Android 7+ security configurations prevent 80-90% of apps from trusting user-installed certificates.
* iOS local VPN approaches provide approximately 70% traffic visibility, limited only by certificate pinning.
* Cloud-based device farms are the only viable solution for high-coverage HTTPS body capture without app modification.
* Metadata-only capture is the only zero-friction method currently available for modern Android devices.

### [data-ingestion](https://github.com/Kahtaf/research/tree/main/data-ingestion) (2025-11-12)

Researchers evaluated strategies for ingesting data from multiple sources while ensuring cryptographic provenance guarantees

<!--[[[end]]]-->

---

## Updating this README

This README uses [cogapp](https://nedbatchelder.com/code/cog/) to automatically generate project descriptions.

### Automatic updates

A GitHub Action automatically runs `cog -r -P README.md` on every push to main and commits any changes to the README or new `_summary.md` files.

### Manual updates

To update locally:

```bash
# Run cogapp to regenerate the project list
cog -r -P README.md
```

The script automatically:
- Discovers all subdirectories in this folder
- Gets the first commit date for each folder and sorts by most recent first
- For each folder, checks if a `_summary.md` file exists
- If the summary exists, it uses the cached version
- If not, it generates a new summary using `llm -m <!--[[[cog
print(MODEL, end='')
]]]-->
gemini-3-flash-preview
<!--[[[end]]]-->` with a prompt that creates engaging descriptions with bullets and links
- Creates markdown links to each project folder on GitHub
- New summaries are saved to `_summary.md` to avoid regenerating them on every run

To regenerate a specific project's description, delete its `_summary.md` file and run `cog -r -P README.md` again.
