# rod-cli: Chrome Automation from the Command Line

A Go CLI tool that drives a persistent headless Chrome instance using the [rod](https://github.com/go-rod/rod) browser automation library. Each command connects to the same long-running Chrome process, making it easy to script multi-step browser interactions from shell scripts or interactive use.

## Architecture

```
rod-cli start     →  launches Chrome (headless, persists after CLI exits)
                     saves WebSocket debug URL to ~/.rod-cli/state.json

rod-cli open URL  →  connects to running Chrome via WebSocket
                     navigates the active tab, disconnects

rod-cli js EXPR   →  connects, evaluates JS, prints result, disconnects

rod-cli stop      →  connects and shuts down Chrome, cleans up state
```

Each CLI invocation is a short-lived process. Chrome runs independently and tabs persist between commands.

## Building

```bash
cd go-rod-cli
go build -o rod-cli .
```

Requires:
- Go 1.21+
- Google Chrome or Chromium installed (or set `ROD_CHROME_BIN=/path/to/chrome`)

## Usage

### Start/stop the browser

```bash
rod-cli start          # Launch headless Chrome
rod-cli status         # Show browser info and active page
rod-cli stop           # Shut down Chrome
```

### Navigate

```bash
rod-cli open https://example.com    # Navigate to URL
rod-cli open example.com            # http:// prefix added automatically
rod-cli back                        # Go back
rod-cli forward                     # Go forward
rod-cli reload                      # Reload page
```

### Extract information

```bash
rod-cli url                    # Print current URL
rod-cli title                  # Print page title
rod-cli text "h1"              # Print text content of element
rod-cli html "div.content"     # Print outer HTML of element
rod-cli html                   # Print full page HTML
rod-cli attr "a#link" href     # Print attribute value
rod-cli pdf output.pdf         # Save page as PDF
```

### Run JavaScript

```bash
rod-cli js document.title                        # Evaluate expression
rod-cli js "1 + 2"                               # Math
rod-cli js 'document.querySelector("h1").textContent'  # DOM queries
rod-cli js '[1,2,3].map(x => x * 2)'            # Returns pretty-printed JSON
rod-cli js 'document.querySelectorAll("a").length'     # Count elements
```

The expression is automatically wrapped in `() => { return (expr); }`.

### Interact with elements

```bash
rod-cli click "button#submit"       # Click element
rod-cli input "#search" "query"     # Type into input field
rod-cli clear "#search"             # Clear input field
rod-cli select "#dropdown" "value"  # Select dropdown by value
rod-cli submit "form#login"         # Submit a form
rod-cli hover ".menu-item"          # Hover over element
rod-cli focus "#email"              # Focus element
```

### Wait for conditions

```bash
rod-cli wait ".loaded"       # Wait for element to appear and be visible
rod-cli waitload             # Wait for page load event
rod-cli waitstable           # Wait for DOM to stop changing
rod-cli waitidle             # Wait for network to be idle
rod-cli sleep 2.5            # Sleep for N seconds
```

### Screenshots

```bash
rod-cli screenshot                      # Save as screenshot.png
rod-cli screenshot page.png             # Save to specific file
rod-cli screenshot-el ".chart" chart.png  # Screenshot specific element
```

### Manage tabs

```bash
rod-cli pages                    # List all tabs (* marks active)
rod-cli newpage https://...      # Open URL in new tab
rod-cli page 1                   # Switch to tab by index
rod-cli closepage 1              # Close tab by index
rod-cli closepage                # Close active tab
```

### Query elements

```bash
rod-cli exists ".loading"    # Exit 0 if exists, exit 1 if not
rod-cli count "li.item"      # Print number of matching elements
rod-cli visible "#modal"     # Exit 0 if visible, exit 1 if not
```

### Shell scripting examples

```bash
# Wait for page to load and extract data
rod-cli start
rod-cli open https://example.com
rod-cli waitstable
title=$(rod-cli title)
echo "Page: $title"

# Conditional logic based on element presence
if rod-cli exists ".error-message"; then
    rod-cli text ".error-message"
fi

# Loop through pages
for url in page1 page2 page3; do
    rod-cli open "https://example.com/$url"
    rod-cli waitstable
    rod-cli screenshot "${url}.png"
done

rod-cli stop
```

## Configuration

| Environment Variable | Default | Description |
|---|---|---|
| `ROD_CHROME_BIN` | `/usr/bin/google-chrome` | Path to Chrome/Chromium binary |
| `ROD_TIMEOUT` | `30` | Default timeout in seconds for element queries |
| `HTTPS_PROXY` / `HTTP_PROXY` | (none) | Authenticated proxy auto-detected on start |

State is stored in `~/.rod-cli/state.json`. Chrome user data is stored in `~/.rod-cli/chrome-data/`.

## Proxy support

In environments with authenticated HTTP proxies (e.g., `HTTPS_PROXY=http://user:pass@host:port`), `rod-cli start` automatically:

1. Detects the proxy credentials from environment variables
2. Launches a local forwarding proxy that injects `Proxy-Authorization` headers into CONNECT requests
3. Configures Chrome to use the local proxy

This is necessary because Chrome cannot natively authenticate to proxies during HTTPS tunnel (CONNECT) establishment. The local proxy runs as a background process and is automatically cleaned up by `rod-cli stop`.

See [claude-code-chrome-proxy.md](claude-code-chrome-proxy.md) for detailed technical notes.

## How it works

The tool uses the [rod](https://github.com/go-rod/rod) Go library which communicates with Chrome via the DevTools Protocol (CDP) over WebSocket. Key implementation details:

- **`start`** uses rod's `launcher` package to start Chrome with `Leakless(false)` so Chrome survives after the CLI exits
- **Proxy auth** handled via a local forwarding proxy that bridges Chrome to authenticated upstream proxies
- **State persistence** via a JSON file containing the WebSocket debug URL and Chrome PID
- **Each command** creates a new rod `Browser` connection to the same Chrome instance, executes the operation, and disconnects
- **Element queries** use rod's built-in auto-wait with a configurable timeout (default 30s)
- **JS evaluation** wraps user expressions in arrow functions as required by rod's `Eval`

## Dependencies

- [github.com/go-rod/rod](https://github.com/go-rod/rod) v0.116.2 - Chrome DevTools Protocol automation

## Commands reference

| Command | Arguments | Description |
|---|---|---|
| `start` | | Launch headless Chrome |
| `stop` | | Shut down Chrome |
| `status` | | Show browser status |
| `open` | `<url>` | Navigate to URL |
| `back` | | Go back in history |
| `forward` | | Go forward in history |
| `reload` | | Reload current page |
| `url` | | Print current URL |
| `title` | | Print page title |
| `html` | `[selector]` | Print HTML (page or element) |
| `text` | `<selector>` | Print element text content |
| `attr` | `<selector> <name>` | Print attribute value |
| `pdf` | `[file]` | Save page as PDF |
| `js` | `<expression>` | Evaluate JavaScript |
| `click` | `<selector>` | Click element |
| `input` | `<selector> <text>` | Type into input |
| `clear` | `<selector>` | Clear input |
| `select` | `<selector> <value>` | Select dropdown value |
| `submit` | `<selector>` | Submit form |
| `hover` | `<selector>` | Hover over element |
| `focus` | `<selector>` | Focus element |
| `wait` | `<selector>` | Wait for element to appear |
| `waitload` | | Wait for page load |
| `waitstable` | | Wait for DOM stability |
| `waitidle` | | Wait for network idle |
| `sleep` | `<seconds>` | Sleep N seconds |
| `screenshot` | `[file]` | Page screenshot |
| `screenshot-el` | `<selector> [file]` | Element screenshot |
| `pages` | | List tabs |
| `page` | `<index>` | Switch tab |
| `newpage` | `[url]` | Open new tab |
| `closepage` | `[index]` | Close tab |
| `exists` | `<selector>` | Check element exists (exit code) |
| `count` | `<selector>` | Count matching elements |
| `visible` | `<selector>` | Check element visible (exit code) |
