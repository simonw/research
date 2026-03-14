# CSRF Protection Demo - Research Notes

## Goal
Demonstrate modern CSRF protection techniques described by Filippo Valsorda and Alex Edwards, using FastAPI, rodney (browser automation), and showboat (executable demo documents).

## Key Concepts from the Articles

### Filippo Valsorda's Article (csrf.md)
- CSRF is a confused deputy attack where attacker causes browser to send requests using victim's cookies
- Modern browsers provide `Sec-Fetch-Site` header indicating request origin relationship
- The recommended 2025 algorithm:
  1. Allow safe methods (GET, HEAD, OPTIONS)
  2. Check Origin against trusted origins allow-list
  3. If Sec-Fetch-Site present: allow `same-origin` or `none`, reject otherwise
  4. If neither Sec-Fetch-Site nor Origin present: allow (not a browser)
  5. If Origin host matches Host header: allow, otherwise reject
- Go 1.25 implements this as `http.CrossOriginProtection`

### Alex Edwards' Article (modern.md)
- `http.CrossOriginProtection` uses Sec-Fetch-Site and Origin headers
- Combined with TLS 1.3 enforcement and SameSite cookies, token-free CSRF protection is viable
- Cross-origin vs cross-site distinction matters
- SameSite=Lax/Strict cookies prevent cross-site but not cross-origin attacks

## Approach
- Build two FastAPI apps: one vulnerable (no CSRF protection), one protected (implementing the modern algorithm)
- Use rodney to automate a real browser making cross-origin requests
- Use showboat to document everything as an executable proof

## Tools Used
- `uvx rodney` - Chrome automation CLI
- `uvx showboat` - Executable demo document builder
- FastAPI + uvicorn for web servers

## Progress
- Cloned gist with both articles
- Read and understood both articles
- Built two FastAPI apps: bank_app.py (target with vulnerable + protected endpoints) and attacker_app.py (malicious site)
- Had to add foo.localhost and bar.localhost to /etc/hosts for DNS resolution
- Discovered that `SameSite=None` requires `Secure=True` in modern Chrome — had to use raw `Set-Cookie` header without SameSite attribute for the "vulnerable" demo
- Successfully demonstrated:
  - CSRF attack succeeding against unprotected endpoint (hidden form auto-submit)
  - CSRF attack blocked by Sec-Fetch-Site middleware on protected endpoint
  - SameSite=Lax cookies preventing cookie transmission on cross-site POSTs
  - Same-origin requests working normally through the middleware
  - Non-browser (curl) requests passing through correctly
- Key finding: The browser sends all the info needed (`Sec-Fetch-Site: cross-site`, `Origin` header) — servers just need to check it
- Used rodney for real Chrome automation, showboat for executable documentation
