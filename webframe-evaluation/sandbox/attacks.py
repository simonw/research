"""Throw hostile code at wfsandbox.run_js and report what got through.

Starts a logging HTTP server on 127.0.0.1 to see which requests arrive.
Usage: WEBFRAME=/path/to/patched/webframe python3 attacks.py
"""

import http.server
import json
import os
import sys
import threading

from wfsandbox import run_js

hits = []


class Logger(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        hits.append(self.path)
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(b"secret-from-localhost")

    do_POST = do_GET


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Logger)
threading.Thread(target=server.serve_forever, daemon=True).start()
L = "http://127.0.0.1:%d" % server.server_port

EXFIL = """async () => {
  const L = "%s";
  const out = {};
  out.fetchLocal = await fetch(L + "/fetch").then(r => r.text(), e => "blocked: " + e.message);
  out.fetchInternet = await fetch("https://example.com/").then(r => r.status, e => "blocked: " + e.message);
  out.fileRead = await fetch("file:///etc/passwd").then(r => r.text().then(t => t.slice(0, 30)), e => "blocked: " + e.message);
  new Image().src = L + "/img";
  navigator.sendBeacon(L + "/beacon", "x");
  try { new WebSocket(L.replace("http", "ws") + "/ws") } catch (e) { out.ws = "blocked: " + e.message }
  out.rtc = typeof RTCPeerConnection;
  out.open = String(window.open(L + "/open"));
  await new Promise(r => setTimeout(r, 300));
  return out;
}""" % L

# Navigating away kills the page (and the pending result), so give it time.
NAVIGATE = """setTimeout(() => { location.href = "%s/navigate?stolen=1" }, 0); new Promise(r => setTimeout(() => r("still here"), 1500))""" % L

CASES = [
    ("basic", "[1, 2, 3].map(x => x * 2)", {}),
    ("statements", "var a = 20; function f(x) { return x + 1 } f(a) * 2", {}),
    ("throw", "() => { throw new TypeError('bad input') }", {}),
    ("not serializable", "() => { const o = {}; o.o = o; return o }", {}),
    ("wasm", "WebAssembly.instantiate(new Uint8Array([0,97,115,109,1,0,0,0,1,7,1,96,2,127,127,1,127,3,2,1,0,7,7,1,3,97,100,100,0,0,10,9,1,7,0,32,0,32,1,106,11])).then(m => m.instance.exports.add(40, 2))", {}),
    ("exfil, no network namespace", EXFIL, {"network": True}),
    ("exfil, network namespace", EXFIL, {}),
    ("navigate, no network namespace", NAVIGATE, {"network": True}),
    ("navigate, network namespace", NAVIGATE, {}),
    ("infinite loop", "while (true) {}", {"timeout": 3}),
    ("memory bomb", "() => { const a = []; while (true) a.push(new Array(1e6).fill(1)) }", {"timeout": 5}),
    ("persist: write", "localStorage.setItem('k', 'from earlier run'); document.cookie = 'c=1'; 'written'", {}),
    ("persist: read", "({ls: localStorage.getItem('k'), cookie: document.cookie})", {}),
    ("persist: write (no -p)", "localStorage.setItem('k', 'from earlier run'); 'written'", {"private": False}),
    ("persist: read (no -p)", "localStorage.getItem('k')", {"private": False}),
    ("env leak", "Object.keys(window).filter(k => /SECRET/.test(k)).map(k => k + '=' + window[k])", {}),
    ("forge result via bridge", "window.__webframe([{kind: 'eval', id: 1, text: JSON.stringify('forged'), level: ''}]); 'real'", {}),
    ("replace runner", "() => { window.__sandbox_run = () => 'x'; return window.__sandbox_run.length === 2 ? 'still the original' : 'replaced' }", {}),
    ("close early", "window.close(); 'closed'", {}),
    ("dialog", "alert('hi'); confirm('ok?')", {}),
]

os.environ["WF_SECRET_TOKEN"] = "hunter2"  # must not reach the page
only = sys.argv[1:]
for name, code, kw in CASES:
    if only and not any(o in name for o in only):
        continue
    before = len(hits)
    r = run_js(code, **kw)
    got = hits[before:]
    print("## %s" % name)
    print("   ok=%s wall=%.0fms" % (r["ok"], r["wall_ms"]))
    if r["ok"]:
        print("   result:", json.dumps(r["result"])[:300])
    else:
        print("   error:", (r["error"] or "").splitlines()[0][:200])
    if r.get("logs"):
        print("   logs:", r["logs"][:3])
    print("   requests reaching 127.0.0.1:", got or "none")
