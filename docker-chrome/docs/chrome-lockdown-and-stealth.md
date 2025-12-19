# Chrome Lockdown & Stealth Configuration Research

Research into making the docker-chrome browser more controlled and less detectable.

## 1. UI Lockdown Options

### 1.1 Disable Developer Console

**Chrome Flags:**
```bash
--disable-dev-tools
```

**Chrome Policy (enterprise):**
```json
{
  "DeveloperToolsAvailability": 2
}
```
- `0` = Allow DevTools
- `1` = Allow except for force-installed extensions
- `2` = Completely disable DevTools

**Limitation:** These can be bypassed by determined users. For true lockdown, use kiosk mode.

### 1.2 Read-Only / Hidden URL Bar

**Kiosk Mode (hides all UI including URL bar):**
```bash
--kiosk
```

**App Mode (minimal UI, no tabs/address bar):**
```bash
--app=https://example.com
```

**Note:** Neither allows a visible-but-read-only URL bar. The URL bar is either fully interactive or hidden. -> let's go with hidden.

**Workaround:** Use `--app` mode and display URL in your control pane UI instead.

### 1.3 Disable Tabs / Single-Tab Mode

**App Mode (single window, no tabs):**
```bash
--app=https://example.com
```

**Disable Popups:**
```bash
--disable-popup-blocking  # Actually allows popups
--block-new-web-contents  # Blocks new tabs/windows
```

**Chrome Policy:**
```json
{
  "URLBlocklist": ["*"],
  "URLAllowlist": ["https://allowed-domain.com/*"]
}
```

### 1.4 Kiosk Mode (Maximum Lockdown)

```bash
chromium \
  --kiosk \
  --no-first-run \
  --disable-infobars \
  --disable-session-crashed-bubble \
  --disable-restore-session-state \
  --noerrdialogs \
  --disable-translate \
  --disable-features=TranslateUI \
  --disable-extensions \
  --disable-component-extensions-with-background-pages \
  --disable-background-networking \
  --disable-sync \
  --disable-default-apps \
  --disable-client-side-phishing-detection \
  "about:blank"
```

**Effect:** Full-screen, no UI chrome, no keyboard shortcuts (F11, Ctrl+T, etc.).

---

## 2. Script Injection Without Extension

### 2.1 Playwright `addInitScript()` ✅ RECOMMENDED

```javascript
// Runs before any page script on every navigation
await page.addInitScript(() => {
  window.myHook = function() { /* ... */ };
  console.log('Injected!');
});

// Or inject from file
await page.addInitScript({ path: './inject.js' });
```

**Scope:** Per-page. Survives navigations within that page object.

### 2.2 CDP `Page.addScriptToEvaluateOnNewDocument` ✅ WORKS

```javascript
const client = await page.context().newCDPSession(page);
await client.send('Page.addScriptToEvaluateOnNewDocument', {
  source: `
    console.log('Injected via CDP!');
    window.__injected = true;
  `
});
```

**Scope:** Persists for the CDP session. Runs on every new document (navigation, iframe).

### 2.3 Context-Level Injection ✅ BEST FOR PERSISTENCE

```javascript
// Applies to ALL pages in the context
await context.addInitScript(() => {
  console.log('Context-level injection');
});
```

**Conclusion:** We do NOT need the Chrome extension for script injection. CDP + Playwright handles it. -> Let's simplify and remove the extension and use CDP + Playwright for script injection.

---

## 3. Network Sniffing Without Extension

### 3.1 CDP Network Domain ✅ ALREADY IMPLEMENTED

```javascript
const client = await page.context().newCDPSession(page);
await client.send('Network.enable');

client.on('Network.requestWillBeSent', (event) => {
  console.log('Request:', event.request.url);
});

client.on('Network.responseReceived', (event) => {
  console.log('Response:', event.response.status);
});

// Get response body
const { body } = await client.send('Network.getResponseBody', {
  requestId: event.requestId
});
```

**Conclusion:** Extension is NOT required for network capture. CDP provides full access.

### 3.2 What the Extension Did (Now Removed)

The extension (`extension/`) was used for:
- Universal content script injection
- Possibly clipboard access

**Status:** Extension has been removed. All functionality now handled via CDP.

---

## 4. Host-to-Browser Clipboard / Pasting Text

### 4.1 Playwright Clipboard API

```javascript
// Write to clipboard (requires secure context)
await page.evaluate(async (text) => {
  await navigator.clipboard.writeText(text);
}, 'text from host');

// Or use CDP
const client = await page.context().newCDPSession(page);
await client.send('Input.insertText', { text: 'pasted text' });
```

### 4.2 CDP Input.insertText ✅ RECOMMENDED

```javascript
// Focus an input first
await page.click('#my-input');

// Then insert text as if typed/pasted
const client = await page.context().newCDPSession(page);
await client.send('Input.insertText', { text: 'Hello from host!' }); 
```

### 4.3 Selkies Clipboard Feature

Selkies has built-in clipboard sync via the UI panel. Users can paste into the "Clipboard" panel and it syncs to the browser.

**For programmatic paste:** Use CDP `Input.insertText`.

-> let's implement whichever is the easiest to paste in text copied from the host to the remote chrome browser

---

## 5. Residential Proxies & Anti-Detection

### 5.1 Why Cloud Run Gets More CAPTCHAs

| Factor | Cloud Run Chrome | Android Emulator |
|--------|------------------|------------------|
| IP Reputation | Google Cloud IPs are flagged as datacenter | Often uses different network |
| User Agent | Desktop Chrome (possibly with automation flags) | Mobile Chrome (trusted) |
| `navigator.webdriver` | `true` (automation detected) | `undefined` (normal) |
| WebGL/Canvas | May have detectable patterns | Normal mobile patterns |
| Behavior | Instant actions, no mouse movement | More human-like |

### 5.2 Anti-Detection Chrome Flags

```bash
chromium \
  --disable-blink-features=AutomationControlled \
  --disable-features=IsolateOrigins,site-per-process \
  --disable-web-security \
  --user-agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
```

### 5.3 Remove navigator.webdriver

```javascript
await page.addInitScript(() => {
  Object.defineProperty(navigator, 'webdriver', {
    get: () => undefined,
  });
});
```

### 5.4 Residential Proxy Configuration

**Chrome Flag:**
```bash
--proxy-server=http://user:pass@proxy.example.com:8080
```

**Playwright:**
```javascript
const browser = await chromium.launch({
  proxy: {
    server: 'http://proxy.example.com:8080',
    username: 'user',
    password: 'pass'
  }
});
```

**For connectOverCDP (our case):**
Chrome must be started with proxy flags. We cannot change proxy after connection.

```bash
# In Dockerfile or start script
chromium \
  --remote-debugging-port=9222 \
  --proxy-server=socks5://residential-proxy.com:1080 \
  --host-resolver-rules="MAP * ~NOTFOUND, EXCLUDE residential-proxy.com"
```

### 5.5 Recommended Stealth Stack

```bash
chromium \
  --no-sandbox \
  --disable-dev-shm-usage \
  --remote-debugging-port=9222 \
  --disable-blink-features=AutomationControlled \
  --disable-features=IsolateOrigins,site-per-process \
  --disable-infobars \
  --disable-extensions \
  --user-agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36" \
  --proxy-server=socks5://your-residential-proxy:1080
```

Plus init script:
```javascript
await context.addInitScript(() => {
  // Remove webdriver flag
  Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
  
  // Fake plugins
  Object.defineProperty(navigator, 'plugins', {
    get: () => [1, 2, 3, 4, 5]
  });
  
  // Fake languages
  Object.defineProperty(navigator, 'languages', {
    get: () => ['en-US', 'en']
  });
});
```

-> let's try to implement this first without the residential proxy

---

## 6. Summary & Recommendations

| Feature | How to Achieve | Extension Needed? |
|---------|----------------|-------------------|
| Disable DevTools | `--disable-dev-tools` or policy | No |
| Hide URL bar | `--kiosk` or `--app` mode | No |
| Disable tabs | `--app` mode | No |
| Script injection | `context.addInitScript()` or CDP | **No** |
| Network sniffing | CDP Network domain | **No** |
| Clipboard paste | CDP `Input.insertText` | No |
| Reduce CAPTCHAs | Residential proxy + stealth flags | No |

### Extension Removal Feasibility

**UPDATE (2025-12-18):** The extension has been removed. All functionality now uses CDP.

The extension (`extension/`) was used for:
1. Content script injection → **Replaced with CDP `Page.addScriptToEvaluateOnNewDocument`**
2. Network interception → **Already using CDP Network domain**

### Implementation Status

| Next Step | Status |
|-----------|--------|
| Remove extension, use CDP-only | ✅ Done |
| Add stealth flags to Chrome startup | ✅ Done |
| Add residential proxy support | 📋 Planned (see `docker-chrome-next-steps/README.md`) |
| Test CAPTCHA rates | ⚠️ Google blocks on IP; bot.sannysoft passes |
