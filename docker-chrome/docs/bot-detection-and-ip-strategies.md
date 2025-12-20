# Bot Detection and IP Strategies

This document covers why Chrome running on different deployment platforms is detected differently by anti-bot systems, and strategies to mitigate detection.

## The Problem

Chrome deployed on **Google Cloud Run** is frequently detected as a bot (presented with captchas), while the same Chrome on a **Google VM** often passes undetected.

## Root Causes

### Cloud Run Detection Issues

1. **Shared Egress IPs**: Cloud Run uses a shared pool of NAT IPs from Google's serverless infrastructure
2. **Known IP Ranges**: Anti-bot services (Cloudflare, DataDome, PerimeterX) maintain lists of Google's serverless IP ranges
3. **IP Reputation**: These IPs are heavily used by scrapers and bots, giving them poor reputation scores
4. **Inconsistent IPs**: NAT IPs can change between requests, a suspicious pattern for real users

### Why VMs Are Less Detected

1. **Dedicated Static IP**: Each VM gets its own external IP address
2. **Different ASN Ranges**: VM IPs come from Google Cloud ranges that aren't as aggressively flagged
3. **Consistent Connection Patterns**: Stable IP over time mimics normal user behavior

## Deployment Strategies

### Option 1: Google Cloud VM (Best for Bot-Sensitive Sites)

Deploy to Compute Engine with a static external IP.

```bash
# Example from deploy-vm.sh
gcloud compute instances create chrome-vm \
  --zone=us-central1-a \
  --machine-type=e2-medium \
  --image-family=debian-12 \
  --image-project=debian-cloud
```

**Pros**: Low cost, stable IP, lower detection rate  
**Cons**: More ops overhead, not serverless

### Option 2: Cloud Run + Residential Proxy (Best Balance)

Use Cloud Run with a residential proxy service to mask the datacenter IP.

```javascript
// Chrome launch args with proxy
'--proxy-server=http://user:pass@brd.superproxy.io:22225'
```

**Pros**: Serverless scaling, residential IP reputation  
**Cons**: Per-GB costs for proxy traffic

### Option 3: Cloud Run Without Proxy (Non-Sensitive Sites Only)

Use for sites without aggressive bot detection.

**Pros**: Simplest, lowest cost  
**Cons**: Will fail on protected sites (Instagram, LinkedIn, etc.)

## Residential Proxy Providers

| Provider | Network Size | Key Features | Best For |
|----------|--------------|--------------|----------|
| **Brightdata** | 72M+ IPs | Unlocker, SERP API, largest network | High-volume, enterprise |
| **Oxylabs** | 100M+ IPs | Direct cloud storage upload, Push-Pull API | Async/batch jobs |
| **SmartProxy** | 55M+ IPs | Developer-friendly, good docs | Smaller scale, startups |
| **Scrapfly** | - | Built-in anti-bot, JS rendering | Simpler integration |

### Current Configuration (Brightdata)

Configured in `deploy.sh`:
```bash
PROXY_HOST="brd.superproxy.io"
PROXY_PORT="22225"
PROXY_USER="your-username"
PROXY_PASS="your-password"
```

> **Important**: Ensure you're using **residential** proxies, not datacenter proxies. Datacenter proxies from these providers are still flagged.

## Decision Matrix

| Target Site | Recommended Deployment | Notes |
|-------------|------------------------|-------|
| Instagram, LinkedIn, TikTok | VM or Cloud Run + Residential Proxy | Aggressive bot detection |
| Google, Amazon | Cloud Run + Residential Proxy | Medium detection |
| General websites | Cloud Run (no proxy) | Usually fine |

## Google Cloud Native Options (Limited)

Google Cloud does **not** offer native residential IP egress. Available options:

- **Cloud NAT with Static IP**: Gives dedicated IP, but still from known datacenter ranges
- **VPC with VPN Gateway**: Requires self-managed residential exit node

## Additional Stealth Measures

Beyond IP strategy, consider:

1. **Browser fingerprint randomization** (see `chrome-lockdown-and-stealth.md`)
2. **Human-like interaction patterns** (mouse movements, timing)
3. **Consistent user agent and viewport**
4. **Proper cookie/session handling**

## Cost Comparison

| Approach | Monthly Cost (est.) | Detection Risk |
|----------|---------------------|----------------|
| Cloud Run (no proxy) | ~$5-20 | High |
| Cloud Run + Brightdata | ~$20-100+ (traffic dependent) | Low |
| VM with static IP | ~$25-50 | Medium |
| VM + Residential Proxy | ~$50+ | Very Low |
