"""
Bank App - A demo "bank" application for CSRF demonstration.

Runs on foo.localhost:8000. Has two modes:
- /vulnerable/* endpoints: No CSRF protection
- /protected/* endpoints: Modern CSRF protection using Sec-Fetch-Site + Origin headers

Both share the same cookie-based "session" for authentication.
"""

import os
import json
import secrets
from fastapi import FastAPI, Request, Response, Form
from fastapi.responses import HTMLResponse, JSONResponse
from starlette.middleware.base import BaseHTTPMiddleware

app = FastAPI(title="Bank App (foo.localhost)")

# In-memory "database"
sessions: dict[str, dict] = {}
balances: dict[str, int] = {"alice": 10000, "bob": 500, "mallory": 100}
transfer_log: list[dict] = []
last_post_headers: dict[str, str] = {}  # Stores headers from the last POST request


# ---------------------------------------------------------------------------
# CSRF Protection Middleware (only applied to /protected/* routes)
# Implements the algorithm from Filippo Valsorda's article
# ---------------------------------------------------------------------------

TRUSTED_ORIGINS: list[str] = []  # Could add e.g. "http://foo.localhost:8000"


class CSRFProtectionMiddleware(BaseHTTPMiddleware):
    """
    Modern CSRF protection using Sec-Fetch-Site and Origin headers.
    Implements Filippo Valsorda's recommended 2025 algorithm.
    """

    async def dispatch(self, request: Request, call_next):
        # Only protect /protected/* routes
        if not request.url.path.startswith("/protected"):
            return await call_next(request)

        # Capture headers for inspection regardless of outcome
        last_post_headers.clear()
        for k, v in request.headers.items():
            if k in ("origin", "host", "referer", "sec-fetch-site", "sec-fetch-mode", "sec-fetch-dest", "sec-fetch-user", "cookie", "content-type"):
                last_post_headers[k] = v

        # Step 1: Allow safe methods
        if request.method in ("GET", "HEAD", "OPTIONS"):
            return await call_next(request)

        # Step 2: Check Origin against trusted origins allow-list
        origin = request.headers.get("origin")
        if origin and origin in TRUSTED_ORIGINS:
            return await call_next(request)

        # Step 3: Check Sec-Fetch-Site header
        sec_fetch_site = request.headers.get("sec-fetch-site")
        if sec_fetch_site is not None:
            if sec_fetch_site in ("same-origin", "none"):
                return await call_next(request)
            else:
                return JSONResponse(
                    status_code=403,
                    content={
                        "error": "CSRF protection: cross-origin request blocked",
                        "sec_fetch_site": sec_fetch_site,
                        "detail": (
                            f"Sec-Fetch-Site was '{sec_fetch_site}', "
                            "but only 'same-origin' or 'none' are allowed "
                            "for non-safe methods."
                        ),
                    },
                )

        # Step 4: If neither Sec-Fetch-Site nor Origin present, allow
        # (not from a modern browser)
        if origin is None:
            return await call_next(request)

        # Step 5: Fallback - compare Origin host with Host header
        from urllib.parse import urlparse

        origin_parsed = urlparse(origin)
        origin_host = origin_parsed.hostname
        if origin_parsed.port:
            origin_host = f"{origin_host}:{origin_parsed.port}"

        request_host = request.headers.get("host", "")

        if origin_host == request_host:
            return await call_next(request)

        return JSONResponse(
            status_code=403,
            content={
                "error": "CSRF protection: origin mismatch",
                "origin": origin,
                "host": request_host,
                "detail": (
                    f"Origin '{origin}' does not match Host '{request_host}'. "
                    "Request blocked."
                ),
            },
        )


app.add_middleware(CSRFProtectionMiddleware)


# ---------------------------------------------------------------------------
# Helper: cookie-based session
# ---------------------------------------------------------------------------

def get_user(request: Request) -> str | None:
    session_id = request.cookies.get("session_id")
    if session_id and session_id in sessions:
        return sessions[session_id]["user"]
    return None


def login_user(response: Response, user: str, samesite: str = "none") -> str:
    session_id = secrets.token_hex(16)
    sessions[session_id] = {"user": user}
    # For the "vulnerable" demo we omit SameSite entirely so the browser
    # uses its default (Lax in modern Chrome, but we use --insecure which
    # may relax that).  We set the cookie with no SameSite attribute
    # and no Secure flag to simulate a legacy app.
    if samesite == "none":
        # Manually set cookie header to avoid Secure requirement
        cookie_str = f"session_id={session_id}; HttpOnly; Path=/"
        response.headers.append("set-cookie", cookie_str)
    else:
        response.set_cookie(
            key="session_id",
            value=session_id,
            httponly=True,
            samesite=samesite,
            secure=False,
            path="/",
        )
    return session_id


# ---------------------------------------------------------------------------
# Shared HTML templates
# ---------------------------------------------------------------------------

def bank_page(user: str, path_prefix: str, protection: str) -> str:
    balance = balances.get(user, 0)
    log_entries = [t for t in transfer_log if t["from"] == user or t["to"] == user]
    log_html = ""
    for t in log_entries[-10:]:
        log_html += f"<li>{t['from']} → {t['to']}: ${t['amount']} ({t['protection']})</li>\n"

    return f"""<!DOCTYPE html>
<html>
<head><title>Bank App ({protection})</title>
<style>
  body {{ font-family: system-ui; max-width: 700px; margin: 40px auto; padding: 0 20px; }}
  .card {{ border: 1px solid #ddd; border-radius: 8px; padding: 20px; margin: 20px 0; }}
  .balance {{ font-size: 2em; color: #2a7; font-weight: bold; }}
  .protection {{ background: {"#d4edda" if protection == "Protected" else "#f8d7da"};
                 color: {"#155724" if protection == "Protected" else "#721c24"};
                 padding: 8px 16px; border-radius: 4px; display: inline-block; }}
  input, select, button {{ padding: 8px 16px; margin: 4px; font-size: 1em; }}
  button {{ background: #007bff; color: white; border: none; border-radius: 4px; cursor: pointer; }}
  button:hover {{ background: #0056b3; }}
  ul {{ padding-left: 20px; }}
</style>
</head>
<body>
  <h1>Demo Bank</h1>
  <p class="protection">{protection} Mode</p>
  <p>Logged in as: <strong>{user}</strong></p>

  <div class="card">
    <h2>Balance</h2>
    <p class="balance">${balance:,}</p>
  </div>

  <div class="card">
    <h2>Transfer Money</h2>
    <form method="POST" action="{path_prefix}/transfer">
      <label>To:
        <select name="to">
          {"".join(f'<option value="{u}">{u}</option>' for u in balances if u != user)}
        </select>
      </label>
      <label>Amount: <input type="number" name="amount" value="100" min="1" /></label>
      <button type="submit">Send</button>
    </form>
  </div>

  <div class="card">
    <h2>Recent Transfers</h2>
    <ul>{log_html if log_html else "<li>No transfers yet</li>"}</ul>
  </div>

  <p><a href="{path_prefix}/logout">Logout</a>
   | <a href="/">Home</a></p>
</body>
</html>"""


# ---------------------------------------------------------------------------
# Home page
# ---------------------------------------------------------------------------

@app.get("/", response_class=HTMLResponse)
async def home(request: Request):
    user = get_user(request)
    login_status = f"Logged in as <strong>{user}</strong>" if user else "Not logged in"
    return f"""<!DOCTYPE html>
<html>
<head><title>CSRF Demo - Bank App</title>
<style>
  body {{ font-family: system-ui; max-width: 700px; margin: 40px auto; padding: 0 20px; }}
  .card {{ border: 1px solid #ddd; border-radius: 8px; padding: 20px; margin: 20px 0; }}
  a {{ color: #007bff; }}
  code {{ background: #f4f4f4; padding: 2px 6px; border-radius: 3px; }}
</style>
</head>
<body>
  <h1>CSRF Demo - Bank App</h1>
  <p>{login_status}</p>

  <div class="card">
    <h2>Login</h2>
    <p>Login as Alice (the victim) to set a session cookie:</p>
    <p>
      <a href="/vulnerable/login?user=alice">Login (Vulnerable - SameSite=None)</a> |
      <a href="/protected/login?user=alice">Login (Protected - SameSite=Lax)</a>
    </p>
  </div>

  <div class="card">
    <h2>Vulnerable Endpoints</h2>
    <p>No CSRF protection. Cookies use <code>SameSite=None</code>.</p>
    <p><a href="/vulnerable/dashboard">Go to Vulnerable Dashboard</a></p>
  </div>

  <div class="card">
    <h2>Protected Endpoints</h2>
    <p>Uses <code>Sec-Fetch-Site</code> + <code>Origin</code> header checking.
       Cookies use <code>SameSite=Lax</code>.</p>
    <p><a href="/protected/dashboard">Go to Protected Dashboard</a></p>
  </div>

  <div class="card">
    <h2>Debug: Request Headers</h2>
    <p><a href="/headers">View current request headers</a></p>
  </div>

  <div class="card">
    <h2>API: Check Balances</h2>
    <p><a href="/api/balances">View all balances (JSON)</a> |
       <a href="/api/transfers">View transfer log (JSON)</a></p>
  </div>
</body>
</html>"""


# ---------------------------------------------------------------------------
# Debug endpoint to see headers
# ---------------------------------------------------------------------------

@app.get("/headers", response_class=HTMLResponse)
async def show_headers(request: Request):
    headers_html = ""
    for key, value in sorted(request.headers.items()):
        highlight = ""
        if key in ("sec-fetch-site", "sec-fetch-mode", "sec-fetch-dest", "origin", "host", "referer"):
            highlight = ' style="background: #fff3cd; font-weight: bold;"'
        headers_html += f"<tr{highlight}><td><code>{key}</code></td><td><code>{value}</code></td></tr>\n"
    return f"""<!DOCTYPE html>
<html>
<head><title>Request Headers</title>
<style>
  body {{ font-family: system-ui; max-width: 700px; margin: 40px auto; }}
  table {{ border-collapse: collapse; width: 100%; }}
  td {{ border: 1px solid #ddd; padding: 8px; }}
</style>
</head>
<body>
  <h1>Request Headers</h1>
  <p>Key CSRF-relevant headers are highlighted.</p>
  <table>{headers_html}</table>
  <p><a href="/">Back</a></p>
</body>
</html>"""


# ---------------------------------------------------------------------------
# API endpoints (JSON)
# ---------------------------------------------------------------------------

@app.get("/api/balances")
async def api_balances():
    return balances


@app.get("/api/transfers")
async def api_transfers():
    return transfer_log


@app.get("/api/last-headers")
async def api_last_headers():
    """Return headers from the most recent POST request."""
    return last_post_headers


@app.post("/api/reset")
async def api_reset():
    """Reset all balances and clear transfer log."""
    balances.clear()
    balances.update({"alice": 10000, "bob": 500, "mallory": 100})
    transfer_log.clear()
    return {"status": "reset"}


# ---------------------------------------------------------------------------
# VULNERABLE endpoints (no CSRF protection, SameSite=None cookies)
# ---------------------------------------------------------------------------

@app.get("/vulnerable/login")
async def vulnerable_login(user: str = "alice"):
    response = HTMLResponse(f"<p>Logged in as {user} (vulnerable mode). <a href='/vulnerable/dashboard'>Dashboard</a></p>")
    login_user(response, user, samesite="none")
    return response


@app.get("/vulnerable/logout")
async def vulnerable_logout():
    response = HTMLResponse("<p>Logged out. <a href='/'>Home</a></p>")
    response.delete_cookie("session_id")
    return response


@app.get("/vulnerable/dashboard", response_class=HTMLResponse)
async def vulnerable_dashboard(request: Request):
    user = get_user(request)
    if not user:
        return HTMLResponse("<p>Not logged in. <a href='/vulnerable/login?user=alice'>Login</a></p>")
    return bank_page(user, "/vulnerable", "Vulnerable")


@app.post("/vulnerable/transfer")
async def vulnerable_transfer(request: Request, to: str = Form(...), amount: int = Form(...)):
    last_post_headers.clear()
    for k, v in request.headers.items():
        if k in ("origin", "host", "referer", "sec-fetch-site", "sec-fetch-mode", "sec-fetch-dest", "sec-fetch-user", "cookie", "content-type"):
            last_post_headers[k] = v
    user = get_user(request)
    if not user:
        return JSONResponse(status_code=401, content={"error": "Not authenticated"})
    if user == to:
        return JSONResponse(status_code=400, content={"error": "Cannot transfer to yourself"})
    if balances.get(user, 0) < amount:
        return JSONResponse(status_code=400, content={"error": "Insufficient funds"})

    balances[user] -= amount
    balances[to] = balances.get(to, 0) + amount
    transfer_log.append({
        "from": user, "to": to, "amount": amount,
        "protection": "NONE",
        "origin": request.headers.get("origin", "N/A"),
        "sec_fetch_site": request.headers.get("sec-fetch-site", "N/A"),
    })

    return HTMLResponse(
        f"<p>Transferred ${amount} to {to}. "
        f"<a href='/vulnerable/dashboard'>Dashboard</a></p>"
    )


# ---------------------------------------------------------------------------
# PROTECTED endpoints (CSRF protection via middleware, SameSite=Lax cookies)
# ---------------------------------------------------------------------------

@app.get("/protected/login")
async def protected_login(user: str = "alice"):
    response = HTMLResponse(f"<p>Logged in as {user} (protected mode). <a href='/protected/dashboard'>Dashboard</a></p>")
    login_user(response, user, samesite="lax")
    return response


@app.get("/protected/logout")
async def protected_logout():
    response = HTMLResponse("<p>Logged out. <a href='/'>Home</a></p>")
    response.delete_cookie("session_id")
    return response


@app.get("/protected/dashboard", response_class=HTMLResponse)
async def protected_dashboard(request: Request):
    user = get_user(request)
    if not user:
        return HTMLResponse("<p>Not logged in. <a href='/protected/login?user=alice'>Login</a></p>")
    return bank_page(user, "/protected", "Protected")


@app.post("/protected/transfer")
async def protected_transfer(request: Request, to: str = Form(...), amount: int = Form(...)):
    # CSRF middleware already ran for /protected/* routes
    last_post_headers.clear()
    for k, v in request.headers.items():
        if k in ("origin", "host", "referer", "sec-fetch-site", "sec-fetch-mode", "sec-fetch-dest", "sec-fetch-user", "cookie", "content-type"):
            last_post_headers[k] = v
    user = get_user(request)
    if not user:
        return JSONResponse(status_code=401, content={"error": "Not authenticated"})
    if user == to:
        return JSONResponse(status_code=400, content={"error": "Cannot transfer to yourself"})
    if balances.get(user, 0) < amount:
        return JSONResponse(status_code=400, content={"error": "Insufficient funds"})

    balances[user] -= amount
    balances[to] = balances.get(to, 0) + amount
    transfer_log.append({
        "from": user, "to": to, "amount": amount,
        "protection": "Sec-Fetch-Site + Origin",
        "origin": request.headers.get("origin", "N/A"),
        "sec_fetch_site": request.headers.get("sec-fetch-site", "N/A"),
    })

    return HTMLResponse(
        f"<p>Transferred ${amount} to {to}. "
        f"<a href='/protected/dashboard'>Dashboard</a></p>"
    )


if __name__ == "__main__":
    import uvicorn
    port = int(os.environ.get("PORT", 8000))
    uvicorn.run(app, host="0.0.0.0", port=port)
