#!/usr/bin/env python3
import argparse
import importlib.util
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import time
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional

BASE = Path(__file__).resolve().parents[1]
ART = BASE / "artifacts"
RES = BASE / "results"
AB_SOCKET_DIR = Path("/tmp/ab-bench")
RUNTIME_DIR = Path("/tmp/ab-runtime")
CAMOUFOX_BIN = Path.home() / ".cache" / "camoufox" / "camoufox-bin"

ART.mkdir(parents=True, exist_ok=True)
RES.mkdir(parents=True, exist_ok=True)
AB_SOCKET_DIR.mkdir(parents=True, exist_ok=True)
RUNTIME_DIR.mkdir(parents=True, exist_ok=True)

URLS = {
    "x": {
        "page_type": "post",
        "url": "https://x.com/jack/status/20",
        "expected": ["post_text", "author_handle", "timestamp", "canonical_url"],
    },
    "reddit": {
        "page_type": "post",
        "url": "https://www.reddit.com/r/Python/comments/10wxbk8/whats_everyone_working_on_this_week/",
        "expected": ["post_title", "post_body", "subreddit", "author", "timestamp", "canonical_url"],
    },
    "linkedin": {
        "page_type": "company",
        "url": "https://www.linkedin.com/company/microsoft/",
        "expected": ["title_or_company", "location", "page_url", "key_metadata"],
    },
    "instagram": {
        "page_type": "profile",
        "url": "https://www.instagram.com/instagram/",
        "expected": ["username", "timestamp", "canonical_url"],
    },
}

COOKIES_RAW = {
    "x": """.x.com\tTRUE\t/\tTRUE\t1804045794\tnight_mode\t2
.x.com\tTRUE\t/\tFALSE\t1807066194\t__cuid\t275a9d3ceb3640b1995368556e0e8f1b
x.com\tFALSE\t/\tFALSE\t0\tlang\ten
.x.com\tTRUE\t/\tTRUE\t1804913448\tpersonalization_id\t\"v1_lI2PyebYalOAhBfGH3Rsfw==\"
.x.com\tTRUE\t/\tTRUE\t1806505040\tdnt\t1
.x.com\tTRUE\t/\tTRUE\t1806505040\tkdt\tSiZbooJQPqWiC4mYFLvUcqv47oC8kbZXCSPWlYTK
.x.com\tTRUE\t/\tTRUE\t1807069795\tauth_multi\t\"1677220568:d10163a806db01f8b460378cefc72968ed0ef76d\"
.x.com\tTRUE\t/\tTRUE\t1806505040\tauth_token\tb905351bfaff2001231b9ce43e16b9a020ea4a7d
.x.com\tTRUE\t/\tTRUE\t1807069800\tguest_id_ads\tv1%3A177194504045419875
.x.com\tTRUE\t/\tTRUE\t1807069800\tguest_id_marketing\tv1%3A177194504045419875
.x.com\tTRUE\t/\tTRUE\t1806505041\tguest_id\tv1%3A177194504045419875
.x.com\tTRUE\t/\tTRUE\t1804045800\ttwid\tu%3D1481080264730288129
.x.com\tTRUE\t/\tTRUE\t1806505041\tct0\t2e1df0c00c40f84751c8193f43e9f97f441b98c588f1550b756237542325325bb951d2a74947c7b32d0abb66d3fd7b1988c4d8c281f4933da2d7592245e8a47c351e1a4f1dcd749e11293a52ddc2e4c8
""",
    "linkedin": """.linkedin.com\tTRUE\t/\tTRUE\t1803697695\tbcookie\t\"v=2&9765043f-0558-4286-8aa3-de7c49100928\"
.www.linkedin.com\tTRUE\t/\tTRUE\t1803697695\tbscookie\t\"v=1&20241223214557c8a2393d-1dad-4ebe-86b8-03f258783920AQFmLjr_--OxwvWkz-67Rt9fYk65vXmc\"
.www.linkedin.com\tTRUE\t/\tTRUE\t1791581959\tli_rm\tAQEk5nVIAGbLrAAAAZP1fEsLj4fYYeuIJfZqzz2SLtQyO82Do_9PCerZNOjv7yC4LSYNxqZ5ccFk7lveh6mcr6TfZvmbpW5vBXTgloeIuE9pJQjNLAs6RBQXhUjUVBq6H5j3rZa6ZWVNKxjJrvWBTU5EfGGJeVWJ42YgMcHXVJK1CvrZV7Und0F5MDt-mPc1ZuJxmvBRUau4TJsZh80QzmYqsyB_F_xYxb1sdkJDe1J8_3kyh0mwayaWD-0-6sXZf4uBlzKs9ii--OLLxZ1C7vLT9wFHGuhGS9O_ZFKO5HOWc3tYMeB52omoXsQg6sBDi2g14fnFtjKQVSjUX3I
.www.linkedin.com\tTRUE\t/\tTRUE\t1803697695\tJSESSIONID\t\"ajax:1957846287504080487\"
.linkedin.com\tTRUE\t/\tTRUE\t1803697695\tliap\ttrue
.www.linkedin.com\tTRUE\t/\tTRUE\t1803697695\tli_at\tAQEDARDtRBUEyys_AAABmcrqBeMAAAGcwR3_6E0AnmjJXw7xTRcC33KHAxzBp7OfsN6zEjMPPRLW8akBqTJHqN1KXtRku1NIBVtnRgpfDUsBZorj2lAJxi4fynv3-tjGEP0FZy_7feEwTNDkmS-uueub
""",
    "instagram": """.instagram.com\tTRUE\t/\tTRUE\t1774480946\tdatr\tMha1Z9A5p_QlLjwHV3MLECiA
.instagram.com\tTRUE\t/\tTRUE\t1774480947\tmid\tZ7UWMgAEAAFmoKDVkfGZj_YuoYV1
.instagram.com\tTRUE\t/\tTRUE\t1803526655\tig_did\t09CD82BC-DDF7-4565-9C65-6283461A74DB
.instagram.com\tTRUE\t/\tTRUE\t1780285828\tds_user_id\t79028106685
.instagram.com\tTRUE\t/\tTRUE\t1807069828\tcsrftoken\tCqBXIT8gwzwFYZXxjW3F26bHeJhibSfN
.instagram.com\tTRUE\t/\tTRUE\t1804045828\tsessionid\t79028106685%3AoIAFrJKUG0TRYj%3A13%3AAYj8V_hmLbkJ84ckqo-XzOAYtFUxABhCoiNymghShg
""",
}

BLOCK_PAT = re.compile(
    r"captcha|challenge|suspicious|unusual traffic|verify|log in|sign in|access denied|temporarily unavailable",
    re.I,
)
SETUP_PATTERNS = [
    (
        re.compile(r"Socket directory .* is not writable", re.I),
        "setup",
        "socket-dir-permission",
        "runtime",
    ),
    (
        re.compile(r"Session name .* is too long", re.I),
        "setup",
        "session-name-too-long",
        "runtime",
    ),
    (
        re.compile(r"Executable doesn't exist at .*ms-playwright", re.I),
        "setup",
        "missing-browser-binary",
        "preflight",
    ),
    (
        re.compile(r"Please run the following command to download new browsers", re.I),
        "setup",
        "missing-browser-binary",
        "preflight",
    ),
    (
        re.compile(r"No module named|ModuleNotFoundError|ImportError", re.I),
        "setup",
        "missing-python-module",
        "preflight",
    ),
]
STARTUP_PATTERNS = [
    (
        re.compile(r"Daemon failed to start", re.I),
        "startup",
        "daemon-start-failed",
        "browser-startup",
    ),
    (
        re.compile(r"Firefox is already running, but is not responding", re.I),
        "startup",
        "profile-lock",
        "browser-startup",
    ),
    (
        re.compile(r"Target page, context or browser has been closed", re.I),
        "startup",
        "browser-closed",
        "browser-startup",
    ),
]
TIMEOUT_PATTERNS = [
    (
        re.compile(r"Timeout \d+ms exceeded|timeout", re.I),
        "timeout",
        "navigation-timeout",
        "navigation",
    )
]


def parse_cookies(site: str) -> List[Dict[str, Any]]:
    out = []
    raw = COOKIES_RAW.get(site, "").strip()
    if not raw:
        return out
    for ln in raw.splitlines():
        parts = ln.split("\t")
        if len(parts) < 7:
            continue
        domain, include_sub, path, secure, exp, name, value = parts[:7]
        ck = {
            "name": name,
            "value": value.strip('"'),
            "domain": domain,
            "path": path,
            "secure": secure == "TRUE",
        }
        if exp.isdigit() and int(exp) > 0:
            ck["expires"] = int(exp)
        out.append(ck)
    return out


def cookie_log_summary(cookies: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    summary = []
    for cookie in cookies:
        summary.append(
            {
                "name": cookie.get("name", ""),
                "domain": cookie.get("domain", ""),
                "path": cookie.get("path", ""),
                "secure": bool(cookie.get("secure")),
                "expires": cookie.get("expires"),
            }
        )
    return summary


def write_storage_state(path: Path, cookies: List[Dict[str, Any]]) -> None:
    path.write_text(json.dumps({"cookies": cookies, "origins": []}, indent=2))


def log_cookie_import(adir: Path, tool: str, site: str, cookies: List[Dict[str, Any]], extra: Optional[Dict[str, Any]] = None) -> None:
    payload: Dict[str, Any] = {
        "tool": tool,
        "site": site,
        "cookie_count": len(cookies),
        "cookies": cookie_log_summary(cookies),
    }
    if extra:
        payload.update(extra)
    append_log(adir / "commands.log", "cookie-import", json.dumps(payload, indent=2))


def extract(site: str, text: str, url: str) -> Dict[str, str]:
    t = text or ""
    e: Dict[str, str] = {}
    if site == "x":
        e["post_text"] = next(iter(re.findall(r'"text":"([^"]{10,280})"', t)), "") or next(
            iter(re.findall(r"\n(.{20,280})\n.*?@", t)),
            "",
        )
        e["author_handle"] = next(iter(re.findall(r"@([A-Za-z0-9_]{1,15})", t)), "")
        e["timestamp"] = next(
            iter(re.findall(r"\d{1,2}:\d{2}\s?(?:AM|PM)?.*?\d{4}|\d{4}-\d{2}-\d{2}T[^\s\"]+", t)),
            "",
        )
        e["canonical_url"] = url if "x.com" in (url or "") else ""
    elif site == "reddit":
        e["post_title"] = next(iter(re.findall(r"<title>(.*?)</title>", t, re.I | re.S)), "").strip()
        e["post_body"] = next(iter(re.findall(r"<shreddit-post[\s\S]*?>", t, re.I)), "")[:300]
        e["subreddit"] = next(iter(re.findall(r"r/([A-Za-z0-9_]+)", t)), "")
        e["author"] = next(iter(re.findall(r"u/([A-Za-z0-9_\-]+)", t)), "")
        e["timestamp"] = next(
            iter(re.findall(r"\d+\s+(?:hours?|days?|minutes?)\s+ago|\d{4}-\d{2}-\d{2}T[^\s\"]+", t)),
            "",
        )
        e["canonical_url"] = url if "reddit.com" in (url or "") else ""
    elif site == "linkedin":
        e["title_or_company"] = next(iter(re.findall(r"Microsoft|Company|LinkedIn", t, re.I)), "")
        e["location"] = next(iter(re.findall(r"Redmond|United States|Toronto|Remote", t, re.I)), "")
        e["page_url"] = url if "linkedin.com" in (url or "") else ""
        e["key_metadata"] = next(iter(re.findall(r"\d+[+,]?\s+employees|Information Technology", t, re.I)), "")
    elif site == "instagram":
        e["username"] = next(iter(re.findall(r"instagram", t, re.I)), "")
        e["timestamp"] = next(iter(re.findall(r"\d{4}-\d{2}-\d{2}T[^\s\"]+", t)), "")
        e["canonical_url"] = url if "instagram.com" in (url or "") else ""
    return e


def json_dump(path: Path, payload: Any) -> None:
    path.write_text(json.dumps(payload, indent=2))


def read_text(path: Path) -> str:
    return path.read_text(errors="ignore") if path.exists() else ""


def append_log(path: Path, label: str, content: str) -> None:
    existing = path.read_text(errors="ignore") if path.exists() else ""
    existing += f"\n===== {label} =====\n"
    existing += content if content.endswith("\n") else f"{content}\n"
    path.write_text(existing)


def remediation_for(tool: str, reason: str) -> List[str]:
    if tool == "agent-browser":
        if reason == "missing-executable":
            return ["npm install -g agent-browser", "agent-browser install"]
        if reason == "socket-dir-permission":
            return [f"mkdir -p {AB_SOCKET_DIR}", f"export AGENT_BROWSER_SOCKET_DIR={AB_SOCKET_DIR}"]
        if reason == "daemon-start-failed":
            return ["agent-browser install"]
    if tool == "camofox-browser":
        if reason == "missing-browser-binary":
            return ["camoufox path", "camoufox fetch https://example.com"]
    if tool == "Scrapling":
        if reason == "missing-python-module":
            return ["python3 -m pip install --user scrapling patchright"]
        if reason == "missing-browser-binary":
            return ["python3 -m patchright install chromium"]
    return []


def detect_patchright_executable() -> Optional[str]:
    if not importlib.util.find_spec("patchright"):
        return None
    try:
        from patchright.sync_api import sync_playwright

        with sync_playwright() as p:
            return p.chromium.executable_path
    except Exception:
        return None


def preflight(verbose: bool = True) -> Dict[str, Any]:
    checks: Dict[str, Any] = {
        "agent-browser": {"ok": True, "issues": [], "remediation": []},
        "camofox-browser": {"ok": True, "issues": [], "remediation": []},
        "Scrapling": {"ok": True, "issues": [], "remediation": []},
    }

    if shutil.which("agent-browser") is None:
        checks["agent-browser"]["ok"] = False
        checks["agent-browser"]["issues"].append("agent-browser executable not found on PATH")
        checks["agent-browser"]["remediation"].extend(remediation_for("agent-browser", "missing-executable"))

    if not AB_SOCKET_DIR.exists():
        AB_SOCKET_DIR.mkdir(parents=True, exist_ok=True)
    if not os.access(AB_SOCKET_DIR, os.W_OK):
        checks["agent-browser"]["ok"] = False
        checks["agent-browser"]["issues"].append(f"agent-browser socket dir is not writable: {AB_SOCKET_DIR}")
        checks["agent-browser"]["remediation"].extend(remediation_for("agent-browser", "socket-dir-permission"))

    if not CAMOUFOX_BIN.exists():
        checks["camofox-browser"]["ok"] = False
        checks["camofox-browser"]["issues"].append(f"Camoufox binary not found: {CAMOUFOX_BIN}")
        checks["camofox-browser"]["remediation"].extend(remediation_for("camofox-browser", "missing-browser-binary"))

    for module_name in ("playwright", "scrapling", "patchright"):
        if not importlib.util.find_spec(module_name):
            checks["Scrapling"]["ok"] = False
            checks["Scrapling"]["issues"].append(f"Python module missing: {module_name}")
            checks["Scrapling"]["remediation"].extend(remediation_for("Scrapling", "missing-python-module"))

    patchright_executable = detect_patchright_executable()
    checks["Scrapling"]["patchright_executable"] = patchright_executable
    if patchright_executable and not Path(patchright_executable).exists():
        checks["Scrapling"]["ok"] = False
        checks["Scrapling"]["issues"].append(f"Patchright browser missing: {patchright_executable}")
        checks["Scrapling"]["remediation"].extend(remediation_for("Scrapling", "missing-browser-binary"))

    if verbose:
        print("== Preflight ==", flush=True)
        for tool, result in checks.items():
            status = "OK" if result["ok"] else "ACTION REQUIRED"
            print(f"[{status}] {tool}", flush=True)
            for issue in result["issues"]:
                print(f"  - {issue}", flush=True)
            for step in dict.fromkeys(result["remediation"]):
                print(f"    remediation: {step}", flush=True)
    return checks


def classify_runtime_failure(tool: str, stdout: str, stderr: str, error: str) -> Dict[str, str]:
    blob = "\n".join(x for x in [stdout, stderr, error] if x)
    for pattern, category, reason, stage in SETUP_PATTERNS:
        if pattern.search(blob):
            return {"failure_category": category, "failure_reason": reason, "failure_stage": stage}
    for pattern, category, reason, stage in STARTUP_PATTERNS:
        if pattern.search(blob):
            return {"failure_category": category, "failure_reason": reason, "failure_stage": stage}
    for pattern, category, reason, stage in TIMEOUT_PATTERNS:
        if pattern.search(blob):
            return {"failure_category": category, "failure_reason": reason, "failure_stage": stage}
    if BLOCK_PAT.search(blob):
        return {"failure_category": "site", "failure_reason": "anti-bot-challenge", "failure_stage": "page"}
    return {"failure_category": "unknown", "failure_reason": "unclassified-error", "failure_stage": "unknown"}


def classify_page(text: str, extracted: Dict[str, str], expected: List[str]) -> Dict[str, str]:
    t = (text or "")[:30000]
    if BLOCK_PAT.search(t):
        return {
            "outcome": "blocked/challenged",
            "failure_category": "site",
            "failure_reason": "anti-bot-challenge",
            "failure_stage": "page",
        }
    found = sum(1 for key in expected if extracted.get(key))
    if found == len(expected) and found > 0:
        return {"outcome": "success", "failure_category": "", "failure_reason": "", "failure_stage": ""}
    if found > 0:
        return {
            "outcome": "partial",
            "failure_category": "extraction",
            "failure_reason": "missing-fields",
            "failure_stage": "extraction",
        }
    if t.strip():
        return {
            "outcome": "partial",
            "failure_category": "extraction",
            "failure_reason": "no-expected-fields",
            "failure_stage": "extraction",
        }
    return {
        "outcome": "timeout",
        "failure_category": "timeout",
        "failure_reason": "no-page-content",
        "failure_stage": "navigation",
    }


def build_record(
    tool: str,
    site: str,
    cfg: Dict[str, Any],
    attempt: int,
    cold: bool,
    adir: Path,
    start_time: float,
    outcome: str,
    extracted: Dict[str, str],
    error: str,
    final_url: str = "",
    title: str = "",
    failure_category: str = "",
    failure_reason: str = "",
    failure_stage: str = "",
    remediation: Optional[List[str]] = None,
) -> Dict[str, Any]:
    return {
        "tool": tool,
        "site": site,
        "page_type": cfg["page_type"],
        "attempt": attempt,
        "cold": cold,
        "ts": datetime.now(timezone.utc).isoformat(),
        "run_id": adir.name,
        "duration_s": round(time.time() - start_time, 3),
        "outcome": outcome,
        "extracted": extracted,
        "expected": cfg["expected"],
        "error": error,
        "artifact_dir": str(adir),
        "final_url": final_url,
        "title": title,
        "failure_category": failure_category,
        "failure_reason": failure_reason,
        "failure_stage": failure_stage,
        "remediation": remediation or [],
    }


def run_logged(
    label: str,
    command: List[str],
    adir: Path,
    timeout: int,
    env: Optional[Dict[str, str]] = None,
) -> Dict[str, Any]:
    started = time.time()
    command_str = " ".join(command)
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=timeout,
            env=env,
            cwd=str(BASE),
        )
        result = {
            "returncode": completed.returncode,
            "stdout": completed.stdout or "",
            "stderr": completed.stderr or "",
            "timeout": False,
            "elapsed_s": round(time.time() - started, 3),
            "command": command_str,
        }
    except subprocess.TimeoutExpired as exc:
        result = {
            "returncode": None,
            "stdout": exc.stdout or "",
            "stderr": exc.stderr or "",
            "timeout": True,
            "elapsed_s": round(time.time() - started, 3),
            "command": command_str,
        }
    append_log(adir / "stdout.log", label, result["stdout"])
    append_log(adir / "stderr.log", label, result["stderr"])
    append_log(
        adir / "commands.log",
        label,
        json.dumps(
            {
                "command": result["command"],
                "returncode": result["returncode"],
                "timeout": result["timeout"],
                "elapsed_s": result["elapsed_s"],
            },
            indent=2,
        ),
    )
    return result


def setup_failure_record(
    tool: str,
    site: str,
    cfg: Dict[str, Any],
    attempt: int,
    cold: bool,
    adir: Path,
    start_time: float,
    message: str,
    remediation: List[str],
) -> Dict[str, Any]:
    return build_record(
        tool=tool,
        site=site,
        cfg=cfg,
        attempt=attempt,
        cold=cold,
        adir=adir,
        start_time=start_time,
        outcome="crash/error",
        extracted=extract(site, "", ""),
        error=message,
        failure_category="setup",
        failure_reason="preflight-failed",
        failure_stage="preflight",
        remediation=remediation,
    )


def best_effort_agent_capture(session: str, adir: Path, env: Dict[str, str]) -> Dict[str, str]:
    title = ""
    final_url = ""
    for label, command, target in [
        ("capture-title", ["agent-browser", "--session", session, "get", "title"], adir / "title.txt"),
        ("capture-url", ["agent-browser", "--session", session, "get", "url"], adir / "url.txt"),
        ("capture-shot", ["agent-browser", "--session", session, "screenshot", str(adir / "screen.png")], None),
        ("capture-html", ["agent-browser", "--session", session, "get", "html", "body"], adir / "page.html"),
    ]:
        result = run_logged(label, command, adir, timeout=20, env=env)
        if target and result["returncode"] == 0 and result["stdout"]:
            target.write_text(result["stdout"])
    title = read_text(adir / "title.txt").strip()
    final_url = read_text(adir / "url.txt").strip()
    return {"title": title, "final_url": final_url}


def run_agent_browser(site: str, cfg: Dict[str, Any], attempt: int, cold: bool, checks: Dict[str, Any]) -> Dict[str, Any]:
    run_id = f"agent-browser_{site}_{attempt}_{'cold' if cold else 'warm'}"
    adir = ART / run_id
    adir.mkdir(parents=True, exist_ok=True)
    start_time = time.time()
    if not checks["agent-browser"]["ok"]:
        message = "; ".join(checks["agent-browser"]["issues"])
        rec = setup_failure_record(
            "agent-browser", site, cfg, attempt, cold, adir, start_time, message, checks["agent-browser"]["remediation"]
        )
        json_dump(adir / "record.json", rec)
        return rec

    session = f"ab{site[0]}{attempt}{'c' if cold else 'w'}"
    env = os.environ.copy()
    env["AGENT_BROWSER_SOCKET_DIR"] = str(AB_SOCKET_DIR)
    env["XDG_RUNTIME_DIR"] = str(RUNTIME_DIR)
    existing_args = env.get("AGENT_BROWSER_ARGS", "").strip()
    force_args = "--ozone-platform=x11"
    env["AGENT_BROWSER_ARGS"] = f"{existing_args},{force_args}".strip(",") if existing_args else force_args
    cookies = parse_cookies(site)
    state_path = adir / "agent-browser-state.json"
    write_storage_state(state_path, cookies)
    log_cookie_import(
        adir,
        "agent-browser",
        site,
        cookies,
        extra={"state_path": str(state_path), "import_mode": "storage-state"},
    )

    first_error = ""
    outcome = "crash/error"
    failure_category = "unknown"
    failure_reason = "unclassified-error"
    failure_stage = "unknown"
    rec: Optional[Dict[str, Any]] = None

    steps = [
        ("prime-state", ["agent-browser", "--headed", "--session", session, "--state", str(state_path), "open", "about:blank"], 30),
        ("cookies", ["agent-browser", "--session", session, "--json", "cookies"], 15),
        ("open", ["agent-browser", "--headed", "--session", session, "open", cfg["url"]], 45),
        ("wait", ["agent-browser", "--session", session, "wait", "6000"], 15),
        ("snapshot", ["agent-browser", "--session", session, "snapshot"], 30),
        ("title", ["agent-browser", "--session", session, "get", "title"], 15),
        ("url", ["agent-browser", "--session", session, "get", "url"], 15),
        ("html", ["agent-browser", "--session", session, "get", "html", "body"], 25),
        ("screenshot", ["agent-browser", "--session", session, "screenshot", str(adir / "screen.png")], 25),
    ]

    try:
        for label, command, timeout in steps:
            result = run_logged(label, command, adir, timeout=timeout, env=env)
            if result["timeout"]:
                first_error = f"{label} timed out after {timeout}s"
                failure_category = "timeout"
                failure_reason = "navigation-timeout"
                failure_stage = label
                break
            if result["returncode"] != 0:
                first_error = (
                    result["stderr"].strip()
                    or result["stdout"].strip()
                    or f"agent-browser step {label} exited {result['returncode']}"
                )
                classified = classify_runtime_failure("agent-browser", result["stdout"], result["stderr"], first_error)
                failure_category = classified["failure_category"]
                failure_reason = classified["failure_reason"]
                failure_stage = classified["failure_stage"] or label
                break
            if label == "cookies":
                append_log(adir / "stdout.log", "cookie-check", result["stdout"])
            elif label == "snapshot":
                (adir / "snapshot.txt").write_text(result["stdout"])
            elif label == "title":
                (adir / "title.txt").write_text(result["stdout"])
            elif label == "url":
                (adir / "url.txt").write_text(result["stdout"])
            elif label == "html":
                (adir / "page.html").write_text(result["stdout"])
        capture = best_effort_agent_capture(session, adir, env)
        title = capture["title"]
        final_url = capture["final_url"]
        text = read_text(adir / "page.html") or read_text(adir / "snapshot.txt")
        extracted = extract(site, text, final_url)
        if not first_error:
            classified = classify_page(text, extracted, cfg["expected"])
            outcome = classified["outcome"]
            failure_category = classified["failure_category"]
            failure_reason = classified["failure_reason"]
            failure_stage = classified["failure_stage"]
        elif failure_category == "site" and BLOCK_PAT.search(text):
            outcome = "blocked/challenged"
        elif failure_category == "timeout":
            outcome = "timeout"
        else:
            outcome = "crash/error"
        rec = build_record(
            tool="agent-browser",
            site=site,
            cfg=cfg,
            attempt=attempt,
            cold=cold,
            adir=adir,
            start_time=start_time,
            outcome=outcome,
            extracted=extracted,
            error=first_error,
            final_url=final_url,
            title=title,
            failure_category=failure_category,
            failure_reason=failure_reason,
            failure_stage=failure_stage,
            remediation=remediation_for("agent-browser", failure_reason),
        )
    except Exception as exc:
        append_log(adir / "stderr.log", "agent-browser-exception", repr(exc))
        capture = best_effort_agent_capture(session, adir, env)
        title = capture["title"]
        final_url = capture["final_url"]
        text = read_text(adir / "page.html") or read_text(adir / "snapshot.txt")
        extracted = extract(site, text, final_url)
        classified = classify_runtime_failure("agent-browser", "", "", str(exc))
        rec = build_record(
            tool="agent-browser",
            site=site,
            cfg=cfg,
            attempt=attempt,
            cold=cold,
            adir=adir,
            start_time=start_time,
            outcome="crash/error",
            extracted=extracted,
            error=str(exc),
            final_url=final_url,
            title=title,
            failure_category=classified["failure_category"],
            failure_reason=classified["failure_reason"],
            failure_stage=classified["failure_stage"],
            remediation=remediation_for("agent-browser", classified["failure_reason"]),
        )
    finally:
        run_logged("close", ["agent-browser", "--session", session, "close"], adir, timeout=15, env=env)
    json_dump(adir / "record.json", rec)
    return rec


def run_inline_python(label: str, script: str, adir: Path, timeout: int) -> Dict[str, Any]:
    return run_logged(label, [sys.executable, "-c", script], adir, timeout=timeout, env=os.environ.copy())


def run_camoufox(site: str, cfg: Dict[str, Any], attempt: int, cold: bool, checks: Dict[str, Any]) -> Dict[str, Any]:
    run_id = f"camofox-browser_{site}_{attempt}_{'cold' if cold else 'warm'}"
    adir = ART / run_id
    adir.mkdir(parents=True, exist_ok=True)
    start_time = time.time()
    if not checks["camofox-browser"]["ok"]:
        message = "; ".join(checks["camofox-browser"]["issues"])
        rec = setup_failure_record(
            "camofox-browser", site, cfg, attempt, cold, adir, start_time, message, checks["camofox-browser"]["remediation"]
        )
        json_dump(adir / "record.json", rec)
        return rec

    state_path = BASE / ".profiles" / f"camo-{site}-warm-state.json"
    if cold and state_path.exists():
        state_path.unlink()
    cookies = parse_cookies(site)
    log_cookie_import(
        adir,
        "camofox-browser",
        site,
        cookies,
        extra={"state_path": str(state_path), "import_mode": "context.add_cookies"},
    )
    script = f"""
from pathlib import Path
import json
import os
from playwright.sync_api import sync_playwright

url = {cfg['url']!r}
cookies = {cookies!r}
state_path = Path({str(state_path)!r})
out = Path({str(adir)!r})
bin_path = Path({str(CAMOUFOX_BIN)!r})
os.environ["MOZ_ENABLE_WAYLAND"] = "0"

print(json.dumps({{
    "tool": "camofox-browser",
    "cookie_count": len(cookies),
    "cookies": {cookie_log_summary(cookies)!r},
    "state_path": str(state_path),
    "import_mode": "context.add_cookies",
}}))

with sync_playwright() as p:
    browser = p.firefox.launch(executable_path=str(bin_path), headless=False)
    context_kwargs = {{}}
    if state_path.exists():
        context_kwargs["storage_state"] = str(state_path)
    context = browser.new_context(**context_kwargs)
    if cookies:
        context.add_cookies(cookies)
    page = context.new_page()
    page.goto(url, wait_until="domcontentloaded", timeout=45000)
    page.wait_for_timeout(6000)
    (out / "url.txt").write_text(page.url)
    (out / "title.txt").write_text(page.title())
    (out / "page.html").write_text(page.content())
    page.screenshot(path=str(out / "screen.png"), full_page=True)
    context.storage_state(path=str(state_path))
    context.close()
    browser.close()
"""
    result = run_inline_python("camoufox", script, adir, timeout=85)
    text = read_text(adir / "page.html")
    final_url = read_text(adir / "url.txt").strip()
    title = read_text(adir / "title.txt").strip()
    extracted = extract(site, text, final_url)

    if result["timeout"]:
        outcome = "timeout"
        failure_category = "timeout"
        failure_reason = "navigation-timeout"
        failure_stage = "navigation"
        error = "camoufox timed out"
    elif result["returncode"] != 0:
        error = result["stderr"].strip() or result["stdout"].strip() or f"camofox exit {result['returncode']}"
        classified = classify_runtime_failure("camofox-browser", result["stdout"], result["stderr"], error)
        outcome = "blocked/challenged" if classified["failure_category"] == "site" else "crash/error"
        failure_category = classified["failure_category"]
        failure_reason = classified["failure_reason"]
        failure_stage = classified["failure_stage"]
    else:
        classified = classify_page(text, extracted, cfg["expected"])
        outcome = classified["outcome"]
        failure_category = classified["failure_category"]
        failure_reason = classified["failure_reason"]
        failure_stage = classified["failure_stage"]
        error = ""

    rec = build_record(
        tool="camofox-browser",
        site=site,
        cfg=cfg,
        attempt=attempt,
        cold=cold,
        adir=adir,
        start_time=start_time,
        outcome=outcome,
        extracted=extracted,
        error=error,
        final_url=final_url,
        title=title,
        failure_category=failure_category,
        failure_reason=failure_reason,
        failure_stage=failure_stage,
        remediation=remediation_for("camofox-browser", failure_reason),
    )
    json_dump(adir / "record.json", rec)
    return rec


def run_scrapling(site: str, cfg: Dict[str, Any], attempt: int, cold: bool, checks: Dict[str, Any]) -> Dict[str, Any]:
    run_id = f"scrapling_{site}_{attempt}_{'cold' if cold else 'warm'}"
    adir = ART / run_id
    adir.mkdir(parents=True, exist_ok=True)
    start_time = time.time()
    if not checks["Scrapling"]["ok"]:
        message = "; ".join(checks["Scrapling"]["issues"])
        rec = setup_failure_record(
            "Scrapling", site, cfg, attempt, cold, adir, start_time, message, checks["Scrapling"]["remediation"]
        )
        json_dump(adir / "record.json", rec)
        return rec

    profile = BASE / ".profiles" / f"scrap-{site}-{'cold' if cold else 'warm'}-{attempt}"
    profile.mkdir(parents=True, exist_ok=True)
    cookies = parse_cookies(site)
    log_cookie_import(
        adir,
        "Scrapling",
        site,
        cookies,
        extra={"profile": str(profile), "import_mode": "StealthySession(cookies=...)"},
    )
    script = f"""
import json
from scrapling.fetchers import StealthySession

url = {cfg['url']!r}
profile = {str(profile)!r}
out = {str(adir)!r}
cookies = {cookies!r}

print(json.dumps({{
    "tool": "Scrapling",
    "cookie_count": len(cookies),
    "cookies": {cookie_log_summary(cookies)!r},
    "profile": profile,
    "import_mode": "StealthySession(cookies=...)",
}}))

with StealthySession(user_data_dir=profile, cookies=cookies) as session:
    response = session.fetch(url, headless=False, timeout=45000, wait=3000)
    open(f"{{out}}/page.html", "w").write(response.text)
    open(f"{{out}}/url.txt", "w").write(getattr(response, "url", url))
"""
    result = run_inline_python("scrapling", script, adir, timeout=90)
    text = read_text(adir / "page.html")
    final_url = read_text(adir / "url.txt").strip()
    extracted = extract(site, text, final_url)

    if result["timeout"]:
        outcome = "timeout"
        failure_category = "timeout"
        failure_reason = "navigation-timeout"
        failure_stage = "navigation"
        error = "scrapling timed out"
    elif result["returncode"] != 0:
        error = result["stderr"].strip() or result["stdout"].strip() or f"scrapling exit {result['returncode']}"
        classified = classify_runtime_failure("Scrapling", result["stdout"], result["stderr"], error)
        outcome = "blocked/challenged" if classified["failure_category"] == "site" else "crash/error"
        failure_category = classified["failure_category"]
        failure_reason = classified["failure_reason"]
        failure_stage = classified["failure_stage"]
    else:
        classified = classify_page(text, extracted, cfg["expected"])
        outcome = classified["outcome"]
        failure_category = classified["failure_category"]
        failure_reason = classified["failure_reason"]
        failure_stage = classified["failure_stage"]
        error = ""

    rec = build_record(
        tool="Scrapling",
        site=site,
        cfg=cfg,
        attempt=attempt,
        cold=cold,
        adir=adir,
        start_time=start_time,
        outcome=outcome,
        extracted=extracted,
        error=error,
        final_url=final_url,
        title="",
        failure_category=failure_category,
        failure_reason=failure_reason,
        failure_stage=failure_stage,
        remediation=remediation_for("Scrapling", failure_reason),
    )
    json_dump(adir / "record.json", rec)
    return rec


def summarize(records: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    grouped: Dict[Any, List[Dict[str, Any]]] = {}
    for record in records:
        grouped.setdefault((record["tool"], record["site"]), []).append(record)

    summary = []
    for (tool, site), rows in grouped.items():
        succ = [x for x in rows if x["outcome"] == "success"]
        partial = [x for x in rows if x["outcome"] == "partial"]
        blocked = [x for x in rows if x["outcome"] == "blocked/challenged"]
        timeout = [x for x in rows if x["outcome"] == "timeout"]
        crash = [x for x in rows if x["outcome"] == "crash/error"]
        succ_times = [x["duration_s"] for x in succ]
        p95 = statistics.quantiles(succ_times, n=20)[-1] if len(succ_times) >= 2 else (succ_times[0] if succ_times else None)

        completeness = []
        correctness = []
        for row in rows:
            expected = len(row["expected"])
            got = sum(1 for field in row["expected"] if row["extracted"].get(field))
            completeness.append(100 * got / expected if expected else 0)
            correctness.append(100 * got / expected if expected else 0)

        failure_reasons = Counter(row.get("failure_reason") for row in rows if row.get("failure_reason"))
        setup_failures = sum(1 for row in rows if row.get("failure_category") == "setup")
        startup_failures = sum(1 for row in rows if row.get("failure_category") == "startup")
        site_failures = sum(1 for row in rows if row.get("failure_category") == "site")
        extraction_failures = sum(1 for row in rows if row.get("failure_category") == "extraction")
        stability = max(0, 100 - (len(timeout) + len(crash)) * 8)

        summary.append(
            {
                "tool": tool,
                "site": site,
                "attempts": len(rows),
                "success": len(succ),
                "partial": len(partial),
                "blocked": len(blocked),
                "timeout": len(timeout),
                "crash": len(crash),
                "success_rate_pct": round(100 * len(succ) / len(rows), 2),
                "block_rate_pct": round(100 * len(blocked) / len(rows), 2),
                "partial_rate_pct": round(100 * len(partial) / len(rows), 2),
                "avg_success_time_s": round(sum(succ_times) / len(succ_times), 3) if succ_times else None,
                "p95_success_time_s": round(p95, 3) if p95 else None,
                "data_completeness_pct": round(sum(completeness) / len(completeness), 2),
                "correctness_spotcheck_pct": round(sum(correctness) / len(correctness), 2),
                "stability_score": round(stability, 2),
                "setup_failures": setup_failures,
                "startup_failures": startup_failures,
                "site_failures": site_failures,
                "extraction_failures": extraction_failures,
                "failure_reasons": dict(failure_reasons),
            }
        )
    return summary


def attempt_plan(mode: str) -> List[Dict[str, Any]]:
    if mode == "sanity":
        return [{"attempt": 1, "cold": True}]
    return [{"attempt": 1, "cold": True}] + [{"attempt": i, "cold": False} for i in range(1, 11)]


def main() -> None:
    parser = argparse.ArgumentParser(description="Run browser automation benchmark")
    parser.add_argument("--mode", choices=["sanity", "full"], default="full")
    parser.add_argument("--tools", nargs="*", choices=["agent-browser", "camofox-browser", "Scrapling"])
    parser.add_argument("--sites", nargs="*", choices=sorted(URLS))
    args = parser.parse_args()

    checks = preflight(verbose=True)
    json_dump(RES / "preflight.json", checks)

    tool_order = args.tools or ["agent-browser", "camofox-browser", "Scrapling"]
    site_order = args.sites or list(URLS.keys())
    fn_by_tool = {
        "agent-browser": run_agent_browser,
        "camofox-browser": run_camoufox,
        "Scrapling": run_scrapling,
    }

    records = []
    for tool in tool_order:
        for site in site_order:
            cfg = URLS[site]
            for plan in attempt_plan(args.mode):
                records.append(fn_by_tool[tool](site, cfg, plan["attempt"], plan["cold"], checks))
            print(f"done {tool} {site}", flush=True)

    json_dump(RES / "attempts.json", records)
    summary = summarize(records)
    json_dump(RES / "summary.json", summary)
    print(f"completed benchmark mode={args.mode}, attempts={len(records)}", flush=True)


if __name__ == "__main__":
    main()
