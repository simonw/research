"""
Prototype: run untrusted JavaScript inside webframe, as a sandbox.

    from wfsandbox import run_js
    run_js("[1, 2, 3].map(x => x * 2)")
    # {'ok': True, 'result': [2, 4, 6], 'logs': [], 'error': None, ...}

Layers (each one optional so their effect can be measured):
- a fresh network namespace (`unshare -rn`): no network at all, not even localhost
- webframe -p: in-memory cookies/storage/cache, nothing persists between runs
- webframe -b: WebKit's bubblewrap sandbox around the web content process
- a page with a strict Content-Security-Policy, a second line of network defence
- a wall-clock timeout, after which the whole process group is killed
- one fresh webframe process per call, so calls can't see each other

Needs the patched webframe binary (for -p and -b); set WEBFRAME=/path/to/it.
"""

import json
import os
import secrets
import shutil
import signal
import subprocess
import sys
import tempfile
import time

WEBFRAME = os.environ.get("WEBFRAME", "webframe")

PAGE = """<!doctype html>
<html><head>
<meta http-equiv="Content-Security-Policy"
  content="default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval'; style-src 'unsafe-inline'">
<script>
(function () {
  // Collect console output in the page instead of letting it reach stdout,
  // where the code could print something that looks like a result.
  var logs = [];
  var stringify = JSON.stringify;
  ["log", "info", "warn", "error", "debug"].forEach(function (m) {
    console[m] = function () {
      logs.push(Array.prototype.map.call(arguments, function (a) {
        try { return typeof a === "string" ? a : stringify(a); } catch (e) { return String(a); }
      }).join(" "));
    };
  });
  // Runs code as a global script. The nonce only lives in this call's
  // arguments, which the evaluated code can't reach.
  Object.defineProperty(window, "__sandbox_run", { value: function (code, nonce) {
    var start = performance.now();
    function done(ok, value) {
      var out;
      try {
        out = stringify({ ok: ok, result: ok ? value : null, error: ok ? null : String(value && value.stack ? value + "\\n" + value.stack : value), logs: logs, ms: performance.now() - start });
      } catch (e) {
        out = stringify({ ok: false, result: null, error: "result is not JSON-serializable: " + e, logs: logs });
      }
      return nonce + out;
    }
    try {
      var r = (0, eval)(code);
      if (typeof r === "function") r = r();
      return Promise.resolve(r).then(function (v) { return done(true, v === undefined ? null : v); },
                                     function (e) { return done(false, e); });
    } catch (e) {
      return done(false, e);
    }
  }});
})();
</script></head><body></body></html>
"""


def run_js(code, timeout=10.0, network=False, private=True, bwrap=True, extra_args=()):
    """Evaluate code in a fresh webframe and return a dict describing the outcome."""
    tmp = tempfile.mkdtemp(prefix="wfsandbox-")
    page = os.path.join(tmp, "sandbox.html")
    with open(page, "w") as f:
        f.write(PAGE)
    nonce = secrets.token_hex(16)
    cmd = [WEBFRAME, "-s", "-c"]
    if private:
        cmd.insert(1, "-p")
    if bwrap and shutil.which("bwrap"):
        cmd.insert(1, "-b")
    cmd += list(extra_args) + [page]
    if not network:
        # -r maps us to root in a new user namespace, so this works unprivileged
        cmd = ["unshare", "-rn"] + cmd
    line = "__sandbox_run(%s, %s)\n" % (json.dumps(code), json.dumps(nonce))
    started = time.monotonic()
    # webframe turns WF_* environment variables into page globals: don't
    # hand the host's to untrusted code.
    env = {k: v for k, v in os.environ.items() if not k.startswith("WF_")}
    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True, start_new_session=True, env=env)
    try:
        out, _ = proc.communicate(line, timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
        proc.communicate()
        return {"ok": False, "result": None, "error": "timeout after %ss" % timeout, "logs": [],
                "wall_ms": (time.monotonic() - started) * 1000}
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    wall_ms = (time.monotonic() - started) * 1000
    # webframe prints the eval result as a JSON string; ours starts with the nonce.
    for raw in out.splitlines():
        try:
            s = json.loads(raw)
        except ValueError:
            continue
        if isinstance(s, str) and s.startswith(nonce):
            result = json.loads(s[len(nonce):])
            result["wall_ms"] = wall_ms
            return result
    return {"ok": False, "result": None, "error": "no result (exit %s)" % proc.returncode,
            "logs": [], "stdout": out[-2000:], "wall_ms": wall_ms}


if __name__ == "__main__":
    code = sys.argv[1] if len(sys.argv) > 1 else sys.stdin.read()
    print(json.dumps(run_js(code), indent=2))
