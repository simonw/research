# Brightdata Proxy SSL/HSTS Issues Analysis

**Date**: 2025-12-21

## Problem Summary

When using Brightdata residential proxy with Chrome in the Cloud Run deployment:
- **Google.com**: Shows "Your connection is not private" error
- **Instagram.com**: Fails with HSTS (HTTP Strict Transport Security) errors

## Root Cause Analysis

### The Core Issue: HTTPS Proxy Interception

Brightdata's residential proxy uses **SSL/TLS interception** (MITM-style) to route HTTPS traffic through their network. This is how they can provide residential IP addresses for encrypted traffic.

When Chrome connects to `https://google.com` through Brightdata:

1. Chrome establishes HTTPS connection to the proxy
2. The proxy terminates SSL and re-encrypts with **Brightdata's CA certificate**
3. Chrome receives a certificate signed by Brightdata, NOT by Google's actual CA
4. Chrome rejects this as an untrusted certificate → **"Connection not private"**

### Why HSTS Makes It Worse

HSTS (HTTP Strict Transport Security) is a security feature where sites like Google and Instagram tell browsers:
- "Only connect to me over HTTPS"
- "Reject any certificate that isn't valid"
- "Remember this forever (or for N years)"

Chrome has a **built-in HSTS preload list** for major sites (google.com, instagram.com, facebook.com, etc.). Even with `--ignore-certificate-errors`, HSTS enforcement can still block connections.

## Current Configuration Issues

### deploy.sh Chrome Flags

```bash
CHROME_ARGS="--remote-debugging-port=9222 --remote-debugging-address=127.0.0.1 --remote-allow-origins=* --no-first-run --start-fullscreen --start-maximized --block-new-web-contents --disable-infobars --disable-extensions --disable-dev-tools --allow-running-insecure-content"
```

**Missing flags for proxy SSL bypass:**
- `--ignore-certificate-errors` - Bypass certificate validation
- `--ignore-certificate-errors-spki-list` - Bypass specific certificates
- `--test-type` - Disables some security features (used internally by Chromium tests)

**The `--allow-running-insecure-content` flag:**
- Only allows mixed HTTP content on HTTPS pages
- Does NOT bypass SSL certificate errors

## Solutions

### Solution 1: Install Brightdata CA Certificate (Recommended)

Brightdata provides a CA certificate that must be installed in Chrome's trust store.

**Download**: https://brightdata.com/static/brightdata_proxy_ca.zip

**Implementation in Dockerfile:**

```dockerfile
# Download and install Brightdata CA certificate
RUN curl -o /tmp/brightdata_ca.zip https://brightdata.com/static/brightdata_proxy_ca.zip && \
    unzip /tmp/brightdata_ca.zip -d /tmp/brightdata_ca && \
    cp /tmp/brightdata_ca/brightdata_proxy_ca_new.crt /usr/local/share/ca-certificates/brightdata.crt && \
    update-ca-certificates && \
    rm -rf /tmp/brightdata_ca.zip /tmp/brightdata_ca

# For Chrome specifically, you may also need to import to NSS database
RUN apt-get update && apt-get install -y libnss3-tools && \
    mkdir -p /home/abc/.pki/nssdb && \
    certutil -d sql:/home/abc/.pki/nssdb -N --empty-password && \
    certutil -d sql:/home/abc/.pki/nssdb -A -t "C,," -n "Brightdata CA" -i /usr/local/share/ca-certificates/brightdata.crt && \
    chown -R abc:abc /home/abc/.pki
```

**Note**: The linuxserver/chromium image runs as user `abc` (UID 1000).

### Solution 2: Chrome Flags to Bypass SSL Errors (Not Recommended for Production)

Add these flags to `deploy.sh`:

```bash
CHROME_ARGS="${CHROME_ARGS} --ignore-certificate-errors --ignore-ssl-errors --ignore-certificate-errors-spki-list=*"
```

**Caveats:**
- **Security risk**: All SSL validation disabled
- **HSTS bypass may not work**: Chrome's preload list is hardcoded
- **Bot detection**: Sites may detect these flags as automation

### Solution 3: Use Brightdata Port 33335 (New Certificate)

Brightdata recently updated their SSL certificate infrastructure:
- **Old port 22225**: Uses older certificate (expires Sept 2026)
- **New port 33335**: Uses new certificate (expires Sept 2034)

**Update your proxy configuration:**

```bash
# Instead of:
PROXY_SERVER=brd.superproxy.io:22225

# Use:
PROXY_SERVER=brd.superproxy.io:33335
```

### Solution 4: KYC Verification (No Certificate Required)

According to Brightdata docs, you can bypass the SSL certificate requirement entirely:

1. Complete KYC (Know Your Customer) verification on your Brightdata account
2. Once verified, the proxy works without SSL interception for HTTPS

This is the **cleanest solution** but requires account verification.

### Solution 5: Use Non-MITM Proxy (Alternative Providers)

Some proxy providers offer **transparent HTTPS proxying** via CONNECT method that doesn't require certificate installation:

| Provider | MITM Required? | Notes |
|----------|----------------|-------|
| **Brightdata** | Yes (or KYC) | Residential, requires CA cert or KYC |
| **Oxylabs** | Yes | Similar to Brightdata |
| **Smartproxy** | Depends on plan | Some modes don't require cert |
| **IPRoyal** | No (some plans) | Simpler integration |
| **Webshare** | No | Datacenter + Residential options |

**Note**: Providers that DON'T intercept SSL typically:
- Use CONNECT tunneling (no content inspection)
- May have fewer "unlocker" features
- Are simpler to integrate

## Recommended Implementation Plan

### Immediate Fix: Install Brightdata CA Certificate

1. **Update Dockerfile**:

```dockerfile
FROM linuxserver/chromium:latest

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    gnupg \
    nodejs \
    socat \
    unzip \
    libnss3-tools \
    && rm -rf /var/lib/apt/lists/*

# Download and install Brightdata CA certificate
RUN curl -L -o /tmp/brightdata_ca.zip https://brightdata.com/static/brightdata_proxy_ca.zip && \
    unzip /tmp/brightdata_ca.zip -d /tmp/brightdata_ca && \
    cp /tmp/brightdata_ca/brightdata_proxy_ca_new.crt /usr/local/share/ca-certificates/brightdata.crt && \
    update-ca-certificates && \
    rm -rf /tmp/brightdata_ca.zip /tmp/brightdata_ca

# ... rest of Dockerfile
```

2. **Add Chrome certificate import script** (runs at container start):

Create `/custom-cont-init.d/install-brightdata-cert.sh`:

```bash
#!/bin/bash
# Install Brightdata CA to Chrome's NSS database
CERT_PATH="/usr/local/share/ca-certificates/brightdata.crt"
NSS_DIR="/config/.pki/nssdb"

if [ -f "$CERT_PATH" ]; then
    mkdir -p "$NSS_DIR"
    if [ ! -f "$NSS_DIR/cert9.db" ]; then
        certutil -d sql:"$NSS_DIR" -N --empty-password
    fi
    certutil -d sql:"$NSS_DIR" -A -t "C,," -n "Brightdata CA" -i "$CERT_PATH" 2>/dev/null || true
    chown -R abc:abc /config/.pki
    echo "Brightdata CA certificate installed for Chrome"
fi
```

3. **Update deploy.sh** to use port 33335:

```bash
# Use new Brightdata port with updated certificate
PROXY_SERVER=brd.superproxy.io:33335
```

## Testing Checklist

After implementing the fix:

1. [ ] Visit https://httpbin.org/ip - Should show residential IP
2. [ ] Visit https://google.com - Should load without SSL errors
3. [ ] Visit https://instagram.com - Should load without HSTS errors
4. [ ] Visit https://bot.sannysoft.com - Verify fingerprint tests pass
5. [ ] Check Chrome console for certificate warnings

## Alternative: Quick Test with Ignore Flags

For immediate testing (NOT production), add to `deploy.sh`:

```bash
CHROME_ARGS="${CHROME_ARGS} --ignore-certificate-errors"
```

This bypasses certificate validation but:
- Leaves security vulnerabilities
- May still fail on HSTS-preloaded sites
- Could trigger bot detection

## Summary

| Solution | Effort | Security | Reliability |
|----------|--------|----------|-------------|
| Install Brightdata CA cert | Medium | ✅ Good | ✅ Best |
| Complete KYC verification | Low | ✅ Best | ✅ Best |
| --ignore-certificate-errors | Low | ❌ Poor | ⚠️ Partial |
| Switch to non-MITM provider | High | ✅ Good | Varies |

**Recommendation**: Install the Brightdata CA certificate in the Docker image and use port 33335.
