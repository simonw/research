# go-rod-cli Development Notes

## Environment
- Go 1.24.7 on linux/amd64
- Google Chrome 144.0.7559.132 (installed via deb package)
- Chrome runs headless with `--no-sandbox --disable-gpu`

## Research on rod library
- GitHub: github.com/go-rod/rod (v0.116.2)
- rod auto-downloads Chromium, but we have system Chrome available
- Key patterns: `rod.New().MustConnect()` to launch browser
- `launcher.New().Bin("/usr/bin/google-chrome")` to use system Chrome
- `page.MustNavigate(url).MustWaitLoad()` for navigation
- `page.MustEval(js)` for JavaScript execution (needs function wrapper: `() => expr`)
- `page.MustElement(selector).MustClick()` for clicking
- `page.MustElement(selector).MustText()` for text extraction
- `page.MustScreenshot("file.png")` for screenshots
- Must vs non-Must: Must panics on error, non-Must returns error

## Key discoveries

### Stateful Chrome persistence
- Using `Leakless(false)` is critical - without it, rod's "leakless" feature kills Chrome
  when the parent process (our CLI) exits
- The `launcher.New().MustLaunch()` returns a WebSocket debug URL that persists across
  connections
- Multiple rod clients can connect to the same Chrome instance simultaneously
- Pages/tabs persist between connections - you can open a page in one CLI call and
  interact with it in subsequent calls

### JavaScript evaluation
- rod requires JS expressions wrapped in arrow functions: `() => expr`
- The CLI wraps user expressions automatically: `() => { return (expr); }`
- `page.Eval()` returns a `proto.RuntimeRemoteObject` with `.Value` containing
  a `gson.JSON` object
- For output formatting, check the raw JSON to determine type (string starts with `"`,
  object with `{`, array with `[`)

### Element timeout behavior
- `page.Element(selector)` waits indefinitely for the element to appear
- This is by design (rod auto-waits) but causes CLI to hang for missing elements
- Solution: `page.Timeout(duration)` creates a page clone with a context deadline
- `ROD_TIMEOUT` env var controls the default (30 seconds)

### Network issues
- External URLs fail with `net::ERR_TUNNEL_CONNECTION_FAILED` in this environment
- Tested exclusively with localhost HTTP server
- The tool works fine with local servers and should work with external URLs in
  non-proxied environments

### Chrome connection patterns tested
1. **rod launcher (used)**: `launcher.New().MustLaunch()` - rod manages Chrome process,
   returns debug URL automatically
2. **Manual launch**: Start Chrome with `--remote-debugging-port`, then use
   `launcher.ResolveURL()` to get the WebSocket URL
3. Both approaches support reconnection from separate processes

## Design decisions

### Architecture: CLI-per-command with shared Chrome
Each CLI invocation is a short-lived process that:
1. Reads `~/.rod-cli/state.json` for the Chrome debug URL
2. Connects to the running Chrome via WebSocket
3. Executes the command on the active page
4. Disconnects (without closing Chrome)

This is much simpler than a daemon architecture while still providing statefulness.

### State file (`~/.rod-cli/state.json`)
```json
{
  "debug_url": "ws://127.0.0.1:PORT/devtools/browser/UUID",
  "chrome_pid": 12345,
  "active_page": 0,
  "data_dir": "/root/.rod-cli/chrome-data"
}
```

### Command categories
- **Lifecycle**: start, stop, status
- **Navigation**: open, back, forward, reload
- **Info**: url, title, html, text, attr, pdf
- **Interaction**: js, click, input, clear, select, submit, hover, focus
- **Waiting**: wait, waitload, waitstable, waitidle, sleep
- **Screenshots**: screenshot, screenshot-el
- **Tabs**: pages, page, newpage, closepage
- **Queries**: exists, count, visible

### What worked well
- Single-file Go implementation (~620 lines) - no need for complex package structure
- rod's auto-wait for elements eliminates explicit waits in most cases
- The `js` command with automatic function wrapping makes it easy to run arbitrary JS
- Exit codes for boolean queries (exists, visible) enable use in shell scripts

### Proxy authentication breakthrough
- Environment has JWT-authenticated HTTP proxy (HTTPS_PROXY with user:jwt@host:port)
- `curl` works fine (reads env vars), but Chrome cannot handle proxy auth natively
- Error without proxy support: `net::ERR_TUNNEL_CONNECTION_FAILED`
- Chrome's `--proxy-server` flag doesn't support `user:pass@host:port`
- CDP `Fetch` domain (HandleAuth) doesn't help - operates above the CONNECT tunnel layer
- **Solution**: local forwarding proxy that injects `Proxy-Authorization` into CONNECT requests
- The rod-cli binary acts as its own proxy helper via hidden `_proxy` subcommand
- Also needed `--ignore-certificate-errors` due to cert issues through the proxy
- See `claude-code-chrome-proxy.md` for full details

### What could be improved
- No XPath support yet (rod supports it via `page.ElementX()`)
- No cookie management
- No request interception/network monitoring
- No file upload support
- Could add `--json` output mode for programmatic use
