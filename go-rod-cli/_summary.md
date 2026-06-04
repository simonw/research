Leveraging the [rod](https://github.com/go-rod/rod) browser automation library, rod-cli provides a lightweight Go-based command-line tool for scripting persistent headless Chrome sessions. Each CLI command connects to and manipulates the same long-running Chrome instance via DevTools Protocol, enabling seamless multi-step browser automation in shell scripts or interactive use. State and session data are managed transparently, offering granular control over navigation, DOM extraction, element interaction, tab management, and JavaScript evaluation. The architecture is modular: Chrome persists independently, while individual commands execute as short-lived processes, supporting robust shell scripting and conditional logic.

**Key features:**
- Persistent headless Chrome controlled via CLI for streamlined automation
- Supports navigation, DOM queries, element interaction, screenshots, and PDF export
- Easy scripting in bash: run stepwise browser procedures outside a GUI
- Built with Go; relies on modern Chrome/Chromium and the [rod library](https://github.com/go-rod/rod)
- Maintains session state via JSON and enables advanced tab management

For hands-on usage and examples, see: [rod-cli Project](https://github.com/go-rod/rod-cli)
