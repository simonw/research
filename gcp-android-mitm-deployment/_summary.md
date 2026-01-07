Engineers successfully deployed the [dockerify-android-mitm](https://github.com/sh4hin/dockerify-android-mitm) solution on a GCP virtual machine to facilitate automated network traffic analysis. This environment runs a root-enabled Android 11 emulator within a Docker container, utilizing [mitmproxy](https://mitmproxy.org/) and iptables for seamless, system-wide HTTPS interception. The system provides immediate access to live traffic logs and device interaction via web-based dashboards for both proxy monitoring and real-time screen mirroring.

Key findings and features:
* Verified capture of over 32,000 network flows including decrypted TLSv1.3 traffic.
* Full device interaction enabled via ws-scrcpy on port 8000 and mitmproxy UI on port 8081.
* Automated redirection architecture ensures all application traffic is routed through the proxy without manual configuration.
* Stable deployment confirmed with healthy container status and complete Android boot verification.
