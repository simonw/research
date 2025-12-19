# Stealth Testing Results & Residential Proxy Plan

**Date**: 2025-12-18

## Overview

After removing the Chrome extension and switching to CDP-based injection, we tested the stealth effectiveness of the docker-chrome setup.

## Stealth Testing Results

**Test Environment**: Cloud Run deployment at `https://docker-chrome-432753364585.us-central1.run.app`

| Test | Result | Notes |
|------|--------|-------|
| `navigator.webdriver` | ✅ `false` | Stealth script working |
| `navigator.plugins.length` | ✅ `5` | Faked successfully |
| `navigator.languages` | ✅ `['en-US', 'en']` | Injected correctly |
| `window.chrome.runtime` | ⚠️ `false` | Empty object injected, but `.runtime` is falsy |
| bot.sannysoft.com | ✅ "WebDriver: missing (passed)" | Basic fingerprint test passed |
| Google.com | ❌ Redirected to `/sorry/` captcha | IP reputation block (Cloud Run IP) |

### Key Finding

JavaScript fingerprint evasion works, but Google blocks based on **IP reputation**. Cloud Run egress IPs are flagged as datacenter IPs. A residential proxy is required for Google and other sensitive sites.

### User-Agent Issue

The `--user-agent=...` Chrome flag is not being applied. The actual UA shows `Linux x86_64 Chrome/143` instead of the configured Windows UA. This needs investigation.

---

## Residential Proxy Integration Plan

### Approach

Add proxy support via Chrome CLI flags, configured through environment variables.

### Implementation Steps

#### 1. Environment Variables

Add to `deploy.sh` or Cloud Run configuration:

```bash
PROXY_SERVER=http://user:pass@proxy.example.com:8080
# Or for authenticated proxies:
PROXY_HOST=proxy.example.com
PROXY_PORT=8080
PROXY_USER=username
PROXY_PASS=password
```

#### 2. Chrome Flags

In `server/index.js` → `restartChromeWithNewUserDataDir`:

```javascript
if (process.env.PROXY_SERVER) {
  args.push(`--proxy-server=${process.env.PROXY_SERVER}`);
}
```

#### 3. Authenticated Proxy Handling

Chrome doesn't support inline auth in `--proxy-server` for HTTPS. Options:

| Option | Pros | Cons |
|--------|------|------|
| Local proxy (mitmproxy --upstream) | Works with any auth | Extra process to manage |
| CDP `Fetch.authRequired` | No extra process | Requires code changes |
| Browser extension | Simple | We removed the extension |

**Recommended**: CDP `Fetch.authRequired` handler

#### 4. CDP Auth Handler

Add to `server/index.js`:

```javascript
client.on('Fetch.authRequired', async (params) => {
  await client.send('Fetch.continueWithAuth', {
    requestId: params.requestId,
    authChallengeResponse: {
      response: 'ProvideCredentials',
      username: process.env.PROXY_USER,
      password: process.env.PROXY_PASS
    }
  });
});
await client.send('Fetch.enable', { handleAuthRequests: true });
```

#### 5. Security Considerations

- Never commit credentials to git
- Use Cloud Run secrets: `gcloud run deploy --set-secrets=PROXY_PASS=proxy-pass:latest`
- Rotate credentials regularly
- Consider IP allowlisting on proxy provider

### Testing Plan

1. Set up a residential proxy account (Bright Data, Oxylabs, IPRoyal, etc.)
2. Configure env vars locally or in Cloud Run
3. Navigate to `https://www.google.com/` - should NOT redirect to `/sorry/`
4. Check IP at `https://httpbin.org/ip` - should show residential IP
5. Run bot.sannysoft.com again to confirm fingerprints still pass

---

## Paste UX Improvements

Enhanced the "Paste to Browser" button with visual feedback:

### Changes to `control-panel.tsx`

1. **State Management**:
   - Added `pasteStatus` (`idle` | `success` | `error`)
   - Added `pasteErrorMessage` for error details

2. **UI States**:
   - **Idle**: Default "Paste to Browser" button
   - **Loading**: Spinning icon, button disabled
   - **Success**: Green button with checkmark, "Pasted!" text
   - **Error**: Red button with alert icon, error message

3. **Auto-reset**: Status reverts to idle after 2-3 seconds

---

## Next Actions

- [ ] Implement proxy support in `server/index.js`
- [ ] Add `Fetch.authRequired` handler for authenticated proxies
- [ ] Test with a residential proxy provider
- [ ] Fix `--user-agent` override (currently not being applied)
- [ ] Consider adding more stealth measures (WebGL, Canvas fingerprint)
