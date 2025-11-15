# Quick Reference Card

## Problem
```
The --set web_password='' flag does NOT disable mitmproxy web UI authentication
```

## Root Cause
```
Authentication is MANDATORY in mitmproxy v11.1.2+ (security measure)
CVE-2025-23217 / GHSA-wg33-5h85-7q5p
```

## Solution
```bash
# Change line 54 in entrypoint.sh from:
--set web_password='' \

# To:
--set web_password='mitmproxy' \
```

## Access Web UI
```
URL:      http://localhost:8081
Username: (leave blank)
Password: mitmproxy
```

## Test the Fix
```bash
# Without auth (fails)
curl -i http://localhost:8081
# Expected: 401 Unauthorized

# With auth (works)
curl -i -u ":mitmproxy" http://localhost:8081
# Expected: 200 OK
```

## Files Modified
- `native-app-traffic-capture/android-mitm-mvp/entrypoint.sh`
  - Line 54: Password setting
  - Lines 347-348: Documentation

## Documentation Files
- `README.md` - Complete guide
- `TECHNICAL_SUMMARY.md` - Technical details
- `notes.md` - Investigation log
- `entrypoint.sh.diff` - Git patch
- `EXECUTIVE_SUMMARY.txt` - Management summary
- `INDEX.md` - Navigation guide
- `QUICK_REFERENCE.md` - This file

## Key Facts
| Aspect | Detail |
|--------|--------|
| **Version** | mitmproxy v11.1.2+ |
| **Issue** | Authentication can't be disabled |
| **CVE** | CVE-2025-23217 |
| **Files Changed** | 1 |
| **Code Changes** | 2 |
| **Password** | `mitmproxy` |
| **Security Impact** | None - still enforced |

## Why Empty String Fails
```
--set web_password=''
  ↓
Empty string treated as UNSET
  ↓
mitmproxy generates RANDOM TOKEN
  ↓
Authentication is STILL REQUIRED
```

## Authentication Methods
```bash
# Method 1: Basic Auth
curl -u ":mitmproxy" http://localhost:8081

# Method 2: URL Token
curl "http://localhost:8081/?token=mitmproxy"

# Method 3: Browser Dialog
Navigate to http://localhost:8081
Enter password: mitmproxy
```

## Decision Tree
```
Need to disable authentication?
├─ No → Use fixed password 'mitmproxy' [RECOMMENDED]
└─ Yes
   ├─ Development only? → Use auto-generated tokens
   ├─ Production? → Use reverse proxy with auth
   └─ API access? → Use token in URL parameter
```

## Common Commands

### Rebuild Container
```bash
docker build -t android-mitm-mvp:fixed android-mitm-mvp/
```

### Run Container
```bash
docker run -d --name android-mitm android-mitm-mvp:fixed
```

### Test Access
```bash
docker exec android-mitm curl -u ":mitmproxy" http://localhost:8081
```

### View Startup Output
```bash
docker exec android-mitm grep "Password:" /var/log/startup.log
```

## What NOT to Do
```bash
# ❌ DON'T try to use empty string
--set web_password=''

# ❌ DON'T use --no-auth (doesn't exist)
--no-auth

# ❌ DON'T disable authentication entirely
# (impossible in v11.1.2+)

# ✓ DO use fixed password
--set web_password='mitmproxy'
```

## For Different Passwords
Simply change `'mitmproxy'` to your desired password:

```bash
# In entrypoint.sh line 54:
--set web_password='yourpassword' \

# Update documentation lines 347-348:
echo "     Password: yourpassword"
```

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Still asks for password | This is EXPECTED - auth is mandatory |
| Wrong password | Use `mitmproxy` (no spaces) |
| Can't connect | Check `curl -i http://localhost:8081` |
| Version check | `docker exec container mitmproxy --version` |
| View logs | `docker exec container tail /var/log/mitmproxy.log` |

## More Information

See the detailed documentation:
- 📖 **README.md** - Full explanation and alternatives
- 🔧 **TECHNICAL_SUMMARY.md** - Implementation details
- 📋 **EXECUTIVE_SUMMARY.txt** - Management overview
- 📍 **INDEX.md** - File guide and navigation

---
**Version**: 1.0
**Last Updated**: November 14, 2025
**Status**: Ready to Deploy
