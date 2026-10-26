# Test server: slow pages, CSP pages, and a request logger for exfil tests
import http.server, time, sys, urllib.parse
LOG = open(sys.argv[2] if len(sys.argv) > 2 else "requests.log", "a", buffering=1)
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def send(self, body, ctype="text/html", headers=()):
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        for k, v in headers: self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body.encode() if isinstance(body, str) else body)
    def do_GET(self):
        p = urllib.parse.urlparse(self.path)
        LOG.write(f"GET {self.path}\n")
        if p.path == "/slow":
            self.send_response(200); self.send_header("Content-Type", "text/html"); self.end_headers()
            self.wfile.write(b"<title>slow start</title><p>part1</p>"); self.wfile.flush()
            time.sleep(1.5)
            self.wfile.write(b"<script>document.title='slow done'</script><p>part2</p>")
        elif p.path == "/csp":
            self.send("<title>csp page</title><h1>Hi</h1>", headers=[("Content-Security-Policy", "default-src 'self'; script-src 'self'")])
        elif p.path == "/redirect":
            self.send_response(302); self.send_header("Location", "/csp"); self.end_headers()
        elif p.path == "/404":
            self.send_response(404); self.end_headers(); self.wfile.write(b"<title>nope</title>")
        elif p.path == "/table":
            rows = "".join(f"<tr><td>{i}</td><td>item {i}</td></tr>" for i in range(5))
            self.send(f"<title>Table</title><table id=t>{rows}</table><script>setTimeout(()=>{{document.body.insertAdjacentHTML('beforeend','<div class=late>late content</div>')}}, 800)</script>")
        else:
            self.send("ok", "text/plain", headers=[("Access-Control-Allow-Origin", "*")])
    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        LOG.write(f"POST {self.path} {self.rfile.read(n)[:200]!r}\n")
        self.send("ok", "text/plain", headers=[("Access-Control-Allow-Origin", "*")])
http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
