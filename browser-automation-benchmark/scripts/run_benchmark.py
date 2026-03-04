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

# Allow running directly: python3 scripts/run_benchmark.py
BASE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BASE / "scripts"))

from config import (
    BASE,
    RUNS_DIR,
    AB_SOCKET_DIR,
    RUNTIME_DIR,
    CAMOUFOX_BIN,
    PROFILES,
    URLS,
    COOKIES_RAW,
    BLOCK_PAT,
    FORBIDDEN_PAT,
    SOFT_BLOCK_INDICATORS,
    LOGIN_REDIRECT_PAT,
    SETUP_PATTERNS,
    STARTUP_PATTERNS,
    TIMEOUT_PATTERNS,
    GROUND_TRUTH,
)

from extractors import extract, validate_ground_truth

# Per-run output directories, set in main()
ART: Path = RUNS_DIR
RES: Path = RUNS_DIR

# Headless mode flag, composited in main() from --headless flag and display detection
HEADLESS: bool = True

# Cookie skip flag, set via --no-cookies or during --compare-stealth
NO_COOKIES: bool = False


def parse_cookies(site: str) -> List[Dict[str, Any]]:
    if NO_COOKIES:
        return []
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


def classify_page(text: str, extracted: Dict[str, str], expected: List[str],
                   site: str = "", final_url: str = "") -> Dict[str, str]:
    t = (text or "")[:30000]

    # Always compute block signals for transparency
    full_text = text or ""
    block_signals: List[str] = []
    if BLOCK_PAT.search(t):
        block_signals.append("block_pattern_in_html")
    if FORBIDDEN_PAT.search(full_text):
        block_signals.append("content_forbidden")
    if site and site in SOFT_BLOCK_INDICATORS:
        indicators = SOFT_BLOCK_INDICATORS[site]
        page_size = len(t)
        min_size = indicators.get("min_page_size", 0)
        required = indicators.get("required_elements", [])
        if page_size < min_size and required:
            missing_elements = [el for el in required if el.lower() not in t.lower()]
            if missing_elements:
                block_signals.append("soft_block_indicators")

    # 1. Login redirect (URL-based, always fatal)
    if final_url and LOGIN_REDIRECT_PAT.search(final_url):
        return {
            "outcome": "blocked/challenged",
            "failure_category": "site",
            "failure_reason": "login-redirect",
            "failure_stage": "page",
            "block_signals": block_signals,
        }

    # 2. Extraction success check — all expected fields must be non-empty AND
    #    ground-truth validation (when available) must pass at least 50%.
    #    This prevents garbage extraction from blocked/CAPTCHA pages being
    #    misclassified as success.
    found = sum(1 for key in expected if extracted.get(key))
    if found == len(expected) and found > 0:
        gt = validate_ground_truth(site, extracted)
        correctness = gt.get("correctness_pct")
        if correctness is not None and correctness < 50:
            return {
                "outcome": "blocked/challenged",
                "failure_category": "site",
                "failure_reason": "incorrect-extraction",
                "failure_stage": "extraction",
                "block_signals": block_signals,
            }
        return {
            "outcome": "success",
            "failure_category": "",
            "failure_reason": "",
            "failure_stage": "",
            "block_signals": block_signals,
        }

    # 3. Hard block pattern (only checked when extraction didn't fully succeed)
    if "block_pattern_in_html" in block_signals:
        reason = "content-unavailable" if "content_forbidden" in block_signals else "anti-bot-challenge"
        return {
            "outcome": "blocked/challenged",
            "failure_category": "site",
            "failure_reason": reason,
            "failure_stage": "page",
            "block_signals": block_signals,
        }

    # 4. Soft block check
    if "soft_block_indicators" in block_signals:
        return {
            "outcome": "blocked/challenged",
            "failure_category": "site",
            "failure_reason": "soft-block",
            "failure_stage": "page",
            "block_signals": block_signals,
        }

    # 5. Partial extraction — also check ground truth for garbage data from
    #    blocked pages that managed to extract some fields (e.g. from URL or meta).
    if found > 0:
        gt = validate_ground_truth(site, extracted)
        correctness = gt.get("correctness_pct")
        if correctness is not None and correctness < 50:
            return {
                "outcome": "blocked/challenged",
                "failure_category": "site",
                "failure_reason": "incorrect-extraction",
                "failure_stage": "extraction",
                "block_signals": block_signals,
            }
        return {
            "outcome": "partial",
            "failure_category": "extraction",
            "failure_reason": "missing-fields",
            "failure_stage": "extraction",
            "block_signals": block_signals,
        }
    if t.strip():
        return {
            "outcome": "partial",
            "failure_category": "extraction",
            "failure_reason": "no-expected-fields",
            "failure_stage": "extraction",
            "block_signals": block_signals,
        }
    return {
        "outcome": "timeout",
        "failure_category": "timeout",
        "failure_reason": "no-page-content",
        "failure_stage": "navigation",
        "block_signals": block_signals,
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
    navigation_s: Optional[float] = None,
    extraction_s: Optional[float] = None,
    setup_s: Optional[float] = None,
    step_timings: Optional[Dict[str, float]] = None,
    block_signals: Optional[List[str]] = None,
) -> Dict[str, Any]:
    gt = validate_ground_truth(site, extracted)
    rec: Dict[str, Any] = {
        "tool": tool,
        "site": site,
        "page_type": cfg["page_type"],
        "cookies_mode": "no-cookies" if NO_COOKIES else "cookies",
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
        "ground_truth": gt,
        "block_signals": block_signals or [],
    }
    if navigation_s is not None:
        rec["navigation_s"] = round(navigation_s, 3)
    if extraction_s is not None:
        rec["extraction_s"] = round(extraction_s, 3)
    if setup_s is not None:
        rec["setup_s"] = round(setup_s, 3)
    if step_timings is not None:
        rec["step_timings"] = {k: round(v, 3) for k, v in step_timings.items()}
    return rec


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
        ("capture-html", ["agent-browser", "--session", session, "eval", "document.documentElement.outerHTML"], adir / "page.html"),
    ]:
        result = run_logged(label, command, adir, timeout=20, env=env)
        if target and result["returncode"] == 0 and result["stdout"]:
            stdout = result["stdout"]
            if label == "capture-html":
                try:
                    stdout = json.loads(stdout)
                except (json.JSONDecodeError, TypeError):
                    pass
            target.write_text(stdout)
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
    step_timings: Dict[str, float] = {}

    headed_flag = [] if HEADLESS else ["--headed"]
    if HEADLESS and cold:
        run_logged("close-pre", ["agent-browser", "--session", session, "close"], adir, timeout=10, env=env)
    steps = [
        ("prime-state", ["agent-browser"] + headed_flag + ["--session", session, "--state", str(state_path), "open", "about:blank"], 30),
        ("cookies", ["agent-browser", "--session", session, "--json", "cookies"], 15),
        ("open", ["agent-browser"] + headed_flag + ["--session", session, "open", cfg["url"]], 45),
        ("wait", ["agent-browser", "--session", session, "wait", "6000"], 15),
        ("title", ["agent-browser", "--session", session, "get", "title"], 15),
        ("url", ["agent-browser", "--session", session, "get", "url"], 15),
        ("html", ["agent-browser", "--session", session, "eval", "document.documentElement.outerHTML"], 25),
        ("screenshot", ["agent-browser", "--session", session, "screenshot", str(adir / "screen.png")], 25),
    ]

    try:
        for label, command, timeout in steps:
            result = run_logged(label, command, adir, timeout=timeout, env=env)
            step_timings[label] = result["elapsed_s"]
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
                # If html step fails, try snapshot as fallback
                if label == "html":
                    snap_result = run_logged("snapshot-fallback", ["agent-browser", "--session", session, "snapshot"], adir, timeout=30, env=env)
                    step_timings["snapshot-fallback"] = snap_result["elapsed_s"]
                    if snap_result["returncode"] == 0 and snap_result["stdout"]:
                        (adir / "snapshot.txt").write_text(snap_result["stdout"])
                        first_error = ""  # recovered
                        failure_category = ""
                        failure_reason = ""
                        failure_stage = ""
                        continue
                break
            if label == "cookies":
                append_log(adir / "stdout.log", "cookie-check", result["stdout"])
            elif label == "title":
                (adir / "title.txt").write_text(result["stdout"])
            elif label == "url":
                (adir / "url.txt").write_text(result["stdout"])
            elif label == "html":
                html_out = result["stdout"]
                try:
                    html_out = json.loads(html_out)
                except (json.JSONDecodeError, TypeError):
                    pass
                (adir / "page.html").write_text(html_out)

        # Compute timing breakdown
        setup_s = step_timings.get("prime-state", 0) + step_timings.get("cookies", 0)
        navigation_s = step_timings.get("open", 0) + step_timings.get("wait", 0)

        capture = best_effort_agent_capture(session, adir, env)
        title = capture["title"]
        final_url = capture["final_url"]
        text = read_text(adir / "page.html") or read_text(adir / "snapshot.txt")
        extract_start = time.time()
        extracted = extract(site, text, final_url)
        extraction_s = time.time() - extract_start
        block_signals: List[str] = []
        if not first_error:
            classified = classify_page(text, extracted, cfg["expected"], site=site, final_url=final_url)
            outcome = classified["outcome"]
            failure_category = classified["failure_category"]
            failure_reason = classified["failure_reason"]
            failure_stage = classified["failure_stage"]
            block_signals = classified.get("block_signals", [])
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
            navigation_s=navigation_s,
            extraction_s=extraction_s,
            setup_s=setup_s,
            step_timings=step_timings,
            block_signals=block_signals,
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
            step_timings=step_timings,
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
import time as _time
from camoufox.sync_api import Camoufox

url = {cfg['url']!r}
cookies = {cookies!r}
state_path = Path({str(state_path)!r})
out = Path({str(adir)!r})
os.environ["MOZ_ENABLE_WAYLAND"] = "0"

print(json.dumps({{
    "tool": "camofox-browser",
    "cookie_count": len(cookies),
    "cookies": {cookie_log_summary(cookies)!r},
    "state_path": str(state_path),
    "import_mode": "context.add_cookies",
}}))

headless = {HEADLESS!r}
_nav_start = _time.time()
with Camoufox(headless=headless, humanize=not headless) as browser:
    context = browser.new_context()
    if cookies:
        context.add_cookies(cookies)
    page = context.new_page()
    try:
        page.goto(url, wait_until="domcontentloaded", timeout=45000)
        page.wait_for_timeout(6000)
    finally:
        _nav_end = _time.time()
        try:
            (out / "url.txt").write_text(page.url)
        except Exception:
            pass
        try:
            (out / "title.txt").write_text(page.title())
        except Exception:
            pass
        try:
            (out / "page.html").write_text(page.content())
        except Exception:
            pass
        try:
            page.screenshot(path=str(out / "screen.png"), full_page=True)
        except Exception:
            pass
        try:
            context.storage_state(path=str(state_path))
        except Exception:
            pass
        context.close()
print("__TIMING__" + json.dumps({{"navigation_s": round(_nav_end - _nav_start, 3)}}))
"""
    result = run_inline_python("camoufox", script, adir, timeout=85)

    # Parse timing from subprocess output
    camo_navigation_s = None
    for line in (result.get("stdout") or "").splitlines():
        if line.startswith("__TIMING__"):
            try:
                camo_navigation_s = json.loads(line[len("__TIMING__"):]).get("navigation_s")
            except (json.JSONDecodeError, ValueError):
                pass

    text = read_text(adir / "page.html")
    final_url = read_text(adir / "url.txt").strip()
    title = read_text(adir / "title.txt").strip()
    extract_start = time.time()
    extracted = extract(site, text, final_url)
    extraction_s = time.time() - extract_start

    block_signals: List[str] = []
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
        classified = classify_page(text, extracted, cfg["expected"], site=site, final_url=final_url)
        outcome = classified["outcome"]
        failure_category = classified["failure_category"]
        failure_reason = classified["failure_reason"]
        failure_stage = classified["failure_stage"]
        block_signals = classified.get("block_signals", [])
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
        navigation_s=camo_navigation_s,
        extraction_s=extraction_s,
        block_signals=block_signals,
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
    lock_file = profile / "SingletonLock"
    if lock_file.exists():
        try:
            lock_file.unlink()
        except OSError:
            pass
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
import time as _time
from pathlib import Path
from scrapling.fetchers import StealthySession

url = {cfg['url']!r}
profile = {str(profile)!r}
out = Path({str(adir)!r})
cookies = {cookies!r}

print(json.dumps({{
    "tool": "Scrapling",
    "cookie_count": len(cookies),
    "cookies": {cookie_log_summary(cookies)!r},
    "profile": profile,
    "import_mode": "StealthySession(cookies=...)",
}}))

captured_html = ""

def page_action(page):
    global captured_html
    try:
        page.wait_for_timeout(6000)
    finally:
        try:
            captured_html = page.content()
        except Exception:
            pass
        try:
            page.screenshot(path=str(out / "screen.png"), full_page=True)
        except Exception:
            pass

_nav_start = _time.time()
with StealthySession(user_data_dir=profile, cookies=cookies) as session:
    response = session.fetch(url, headless={HEADLESS!r}, timeout=45000, wait=6000,
                             page_action=page_action)
    _nav_end = _time.time()
    # Try multiple ways to get HTML content
    html = response.text or ""
    if not html:
        html = getattr(response, "html", "") or ""
    if not html:
        html = getattr(response, "body", b"")
        if isinstance(html, bytes):
            html = html.decode("utf-8", errors="ignore")
    # Use page_action capture as fallback
    if not html and captured_html:
        html = captured_html
    (out / "page.html").write_text(html)
    (out / "url.txt").write_text(getattr(response, "url", url))
print("__TIMING__" + json.dumps({{"navigation_s": round(_nav_end - _nav_start, 3)}}))
"""
    result = run_inline_python("scrapling", script, adir, timeout=90)

    # Parse timing from subprocess output
    scrap_navigation_s = None
    for line in (result.get("stdout") or "").splitlines():
        if line.startswith("__TIMING__"):
            try:
                scrap_navigation_s = json.loads(line[len("__TIMING__"):]).get("navigation_s")
            except (json.JSONDecodeError, ValueError):
                pass

    text = read_text(adir / "page.html")
    final_url = read_text(adir / "url.txt").strip()
    extract_start = time.time()
    extracted = extract(site, text, final_url)
    extraction_s = time.time() - extract_start

    block_signals: List[str] = []
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
        classified = classify_page(text, extracted, cfg["expected"], site=site, final_url=final_url)
        outcome = classified["outcome"]
        failure_category = classified["failure_category"]
        failure_reason = classified["failure_reason"]
        failure_stage = classified["failure_stage"]
        block_signals = classified.get("block_signals", [])
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
        navigation_s=scrap_navigation_s,
        extraction_s=extraction_s,
        block_signals=block_signals,
    )
    json_dump(adir / "record.json", rec)
    return rec


def timing_stats(values: List[float]) -> Optional[Dict[str, float]]:
    """Compute descriptive statistics for a list of timing values."""
    if not values:
        return None
    s: Dict[str, float] = {
        "mean": round(statistics.mean(values), 3),
        "median": round(statistics.median(values), 3),
        "min": round(min(values), 3),
        "max": round(max(values), 3),
    }
    if len(values) >= 2:
        s["stdev"] = round(statistics.stdev(values), 3)
        q = statistics.quantiles(values, n=4)
        s["iqr"] = round(q[2] - q[0], 3)
        s["p95"] = round(statistics.quantiles(values, n=20)[-1], 3)
    return s


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

        # Timing stats from all non-crash runs
        all_times = [x["duration_s"] for x in rows if x["outcome"] not in ("crash/error",)]
        succ_times = [x["duration_s"] for x in succ]
        nav_times = [x["navigation_s"] for x in rows if x.get("navigation_s") is not None]
        extract_times = [x["extraction_s"] for x in rows if x.get("extraction_s") is not None]

        completeness = []
        gt_correctness_all = []
        gt_correctness_success = []
        for row in rows:
            expected = len(row["expected"])
            got = sum(1 for field in row["expected"] if row["extracted"].get(field))
            completeness.append(100 * got / expected if expected else 0)
            gt = row.get("ground_truth", {})
            gt_pct = gt.get("correctness_pct")
            if gt_pct is not None:
                gt_correctness_all.append(gt_pct)
                if row["outcome"] == "success":
                    gt_correctness_success.append(gt_pct)

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
                "success_rate": f"{len(succ)}/{len(rows)}",
                "success_rate_pct": round(100 * len(succ) / len(rows), 2),
                "block_rate_pct": round(100 * len(blocked) / len(rows), 2),
                "partial_rate_pct": round(100 * len(partial) / len(rows), 2),
                "timing_total": timing_stats(all_times),
                "timing_success": timing_stats(succ_times),
                "timing_navigation": timing_stats(nav_times),
                "timing_extraction": timing_stats(extract_times),
                "data_completeness_pct": round(sum(completeness) / len(completeness), 2),
                "correctness_pct": round(sum(gt_correctness_all) / len(gt_correctness_all), 2) if gt_correctness_all else None,
                "correctness_success_only_pct": round(sum(gt_correctness_success) / len(gt_correctness_success), 2) if gt_correctness_success else None,
                "stability_score": round(stability, 2),
                "setup_failures": setup_failures,
                "startup_failures": startup_failures,
                "site_failures": site_failures,
                "extraction_failures": extraction_failures,
                "failure_reasons": dict(failure_reasons),
            }
        )
    return summary


def _run_benchmark_loop(
    tool_order: List[str],
    site_order: List[str],
    n_attempts: int,
    fn_by_tool: Dict[str, Any],
    checks: Dict[str, Any],
) -> List[Dict[str, Any]]:
    records: List[Dict[str, Any]] = []
    for tool in tool_order:
        for site in site_order:
            cfg = URLS[site]
            for attempt in range(1, n_attempts + 1):
                cold = (attempt == 1)
                rec = fn_by_tool[tool](site, cfg, attempt, cold, checks)
                records.append(rec)
                print(f"done {tool} {site} attempt={attempt}/{n_attempts} cold={cold} outcome={rec['outcome']}", flush=True)
                if attempt < n_attempts:
                    time.sleep(2)
    return records


def _run_mode_comparison(
    tool_order: List[str],
    site_order: List[str],
    n_attempts: int,
    fn_by_tool: Dict[str, Any],
    checks: Dict[str, Any],
    run_dir: Path,
) -> None:
    global HEADLESS, ART, RES

    mode_results: Dict[str, Any] = {}
    for mode in ("headed", "headless"):
        HEADLESS = (mode == "headless")
        mode_art = run_dir / mode / "artifacts"
        mode_res = run_dir / mode / "results"
        mode_art.mkdir(parents=True, exist_ok=True)
        mode_res.mkdir(parents=True, exist_ok=True)
        ART = mode_art
        RES = mode_res
        print(f"\n== Running in {mode} mode ==", flush=True)

        records = _run_benchmark_loop(tool_order, site_order, n_attempts, fn_by_tool, checks)
        for rec in records:
            rec["mode"] = mode
        json_dump(mode_res / "attempts.json", records)
        summary = summarize(records)
        json_dump(mode_res / "summary.json", summary)
        mode_results[mode] = {"records": records, "summary": summary}

    # Build comparison
    comparison: List[Dict[str, Any]] = []
    headed_by_key = {(s["tool"], s["site"]): s for s in mode_results["headed"]["summary"]}
    headless_by_key = {(s["tool"], s["site"]): s for s in mode_results["headless"]["summary"]}
    for key in headed_by_key:
        h = headed_by_key[key]
        hl = headless_by_key.get(key)
        if not hl:
            continue
        entry: Dict[str, Any] = {
            "tool": key[0],
            "site": key[1],
            "headed_success_rate": h["success_rate"],
            "headless_success_rate": hl["success_rate"],
            "success_rate_diff_pct": round(h["success_rate_pct"] - hl["success_rate_pct"], 2),
        }
        h_nav = h.get("timing_navigation")
        hl_nav = hl.get("timing_navigation")
        if h_nav and hl_nav:
            entry["headed_nav_median"] = h_nav["median"]
            entry["headless_nav_median"] = hl_nav["median"]
            entry["nav_median_diff_s"] = round(h_nav["median"] - hl_nav["median"], 3)
        comparison.append(entry)
    json_dump(run_dir / "mode_comparison.json", comparison)
    print(f"\nMode comparison saved to {run_dir / 'mode_comparison.json'}", flush=True)


def _run_stealth_comparison(
    tool_order: List[str],
    site_order: List[str],
    n_attempts: int,
    fn_by_tool: Dict[str, Any],
    checks: Dict[str, Any],
    run_dir: Path,
) -> None:
    """Run 4 configurations (headed/headless x cookies/no-cookies) and compare."""
    global HEADLESS, NO_COOKIES, ART, RES

    CONFIGS = [
        ("headed-cookies",      False, False),
        ("headed-no-cookies",   False, True),
        ("headless-cookies",    True,  False),
        ("headless-no-cookies", True,  True),
    ]

    mode_results: Dict[str, Any] = {}
    for config_name, headless, no_cookies in CONFIGS:
        HEADLESS = headless
        NO_COOKIES = no_cookies

        # Clean profiles between configs to prevent warm-state cookie leakage
        if PROFILES.exists():
            shutil.rmtree(PROFILES)
            PROFILES.mkdir(parents=True, exist_ok=True)

        mode_art = run_dir / config_name / "artifacts"
        mode_res = run_dir / config_name / "results"
        mode_art.mkdir(parents=True, exist_ok=True)
        mode_res.mkdir(parents=True, exist_ok=True)
        ART = mode_art
        RES = mode_res

        label = f"{'headless' if headless else 'headed'}, {'no cookies' if no_cookies else 'with cookies'}"
        print(f"\n== Running: {label} ==", flush=True)

        records = _run_benchmark_loop(tool_order, site_order, n_attempts, fn_by_tool, checks)
        for rec in records:
            rec["mode"] = config_name
        json_dump(mode_res / "attempts.json", records)
        summary = summarize(records)
        json_dump(mode_res / "summary.json", summary)
        mode_results[config_name] = {"records": records, "summary": summary}

    # Build and save comparison report
    comparison = _build_stealth_comparison(mode_results)
    json_dump(run_dir / "stealth_comparison.json", comparison)
    print(f"\nStealth comparison saved to {run_dir / 'stealth_comparison.json'}", flush=True)


def _build_stealth_comparison(mode_results: Dict[str, Any]) -> Dict[str, Any]:
    """Build a comparison report across all 4 stealth configurations."""
    configs = list(mode_results.keys())

    # Index summaries by (tool, site) for each config
    summaries_by_config: Dict[str, Dict[tuple, Any]] = {}
    for config_name, data in mode_results.items():
        summaries_by_config[config_name] = {
            (s["tool"], s["site"]): s for s in data["summary"]
        }

    # Per tool-site comparison across all 4 modes
    all_keys: set = set()
    for indexed in summaries_by_config.values():
        all_keys.update(indexed.keys())

    per_tool_site: List[Dict[str, Any]] = []
    for tool, site in sorted(all_keys):
        entry: Dict[str, Any] = {"tool": tool, "site": site, "modes": {}}
        for config_name in configs:
            s = summaries_by_config[config_name].get((tool, site))
            if s:
                mode_entry: Dict[str, Any] = {
                    "success_rate": s["success_rate"],
                    "success_rate_pct": s["success_rate_pct"],
                    "block_rate_pct": s["block_rate_pct"],
                    "failure_reasons": s.get("failure_reasons", {}),
                }
                nav = s.get("timing_navigation")
                if nav:
                    mode_entry["nav_median_s"] = nav["median"]
                entry["modes"][config_name] = mode_entry
        per_tool_site.append(entry)

    # Aggregate per-tool across all sites
    per_tool: Dict[str, Dict[str, Dict[str, int]]] = {}
    for config_name, data in mode_results.items():
        for rec in data["records"]:
            tool = rec["tool"]
            per_tool.setdefault(tool, {}).setdefault(config_name, {"total": 0, "success": 0, "blocked": 0})
            per_tool[tool][config_name]["total"] += 1
            if rec["outcome"] == "success":
                per_tool[tool][config_name]["success"] += 1
            elif rec["outcome"] == "blocked/challenged":
                per_tool[tool][config_name]["blocked"] += 1

    tool_summary: List[Dict[str, Any]] = []
    for tool, modes in sorted(per_tool.items()):
        row: Dict[str, Any] = {"tool": tool}
        for config_name, counts in modes.items():
            total = counts["total"]
            row[config_name] = {
                "success_pct": round(100 * counts["success"] / total, 2) if total else 0,
                "block_pct": round(100 * counts["blocked"] / total, 2) if total else 0,
            }
        tool_summary.append(row)

    return {
        "configs": configs,
        "per_tool_site": per_tool_site,
        "tool_summary": tool_summary,
    }


def main() -> None:
    global ART, RES, HEADLESS, NO_COOKIES

    parser = argparse.ArgumentParser(description="Run browser automation benchmark")
    parser.add_argument("--tools", nargs="*", choices=["agent-browser", "camofox-browser", "Scrapling"])
    parser.add_argument("--sites", nargs="*", choices=sorted(URLS))
    parser.add_argument("--name", help="Custom name for this run (default: timestamp)")
    parser.add_argument("--attempts", type=int, default=5,
                        help="Number of attempts per tool/site combination (default: 5)")
    parser.add_argument("--headless", action="store_true", default=False,
                        help="Run in headless mode (default: headed if display available)")
    parser.add_argument("--compare-modes", action="store_true", default=False,
                        help="Run benchmark in both headed and headless modes, then compare")
    parser.add_argument("--no-cookies", action="store_true", default=False,
                        help="Skip cookie loading/injection for all tools")
    parser.add_argument("--compare-stealth", action="store_true", default=False,
                        help="Run 4 configurations (headed/headless x cookies/no-cookies) and compare")
    args = parser.parse_args()

    if args.compare_stealth and args.compare_modes:
        parser.error("--compare-stealth and --compare-modes are mutually exclusive")

    global NO_COOKIES
    if args.no_cookies:
        NO_COOKIES = True
        print("Running without cookies (--no-cookies flag)", flush=True)

    if args.headless:
        HEADLESS = True
        print("Running in headless mode (--headless flag)", flush=True)
    else:
        has_display = bool(os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"))
        # Env vars may be missing in non-login shells (e.g., tmux, SSH, Claude Code).
        # Probe for actual X11/Wayland sockets and set env vars so downstream tools work.
        if not has_display:
            x11_dir = Path("/tmp/.X11-unix")
            wayland_dir = Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}"))
            # Find the user's own X socket (skip greeter sockets owned by other users)
            uid = os.getuid()
            for sock in sorted(x11_dir.glob("X*")):
                try:
                    if sock.stat().st_uid == uid:
                        display = ":" + sock.name[1:]  # X1 -> :1
                        os.environ["DISPLAY"] = display
                        has_display = True
                        print(f"Detected X11 socket {sock}, setting DISPLAY={display}", flush=True)
                        break
                except OSError:
                    continue
            if not has_display:
                for sock in sorted(wayland_dir.glob("wayland-[0-9]*")):
                    if sock.is_socket():
                        os.environ["WAYLAND_DISPLAY"] = sock.name
                        has_display = True
                        print(f"Detected Wayland socket {sock}, setting WAYLAND_DISPLAY={sock.name}", flush=True)
                        break
        if has_display:
            HEADLESS = False
            print("Running in headed mode", flush=True)
        else:
            HEADLESS = True
            print("WARNING: No display detected, falling back to headless mode", flush=True)

    run_name = args.name or datetime.now().strftime("%Y-%m-%d_%H%M%S")
    run_dir = RUNS_DIR / run_name
    ART = run_dir / "artifacts"
    RES = run_dir / "results"
    ART.mkdir(parents=True, exist_ok=True)
    RES.mkdir(parents=True, exist_ok=True)
    print(f"Run directory: {run_dir}", flush=True)

    checks = preflight(verbose=True)
    json_dump(RES / "preflight.json", checks)

    tool_order = args.tools or ["agent-browser", "camofox-browser", "Scrapling"]
    site_order = args.sites or list(URLS.keys())
    n_attempts = args.attempts
    fn_by_tool = {
        "agent-browser": run_agent_browser,
        "camofox-browser": run_camoufox,
        "Scrapling": run_scrapling,
    }

    if args.compare_stealth:
        _run_stealth_comparison(tool_order, site_order, n_attempts, fn_by_tool, checks, run_dir)
    elif args.compare_modes:
        _run_mode_comparison(tool_order, site_order, n_attempts, fn_by_tool, checks, run_dir)
    else:
        records = _run_benchmark_loop(tool_order, site_order, n_attempts, fn_by_tool, checks)
        json_dump(RES / "attempts.json", records)
        summary = summarize(records)
        json_dump(RES / "summary.json", summary)
        print(f"completed benchmark, attempts={len(records)}, run={run_name}", flush=True)


if __name__ == "__main__":
    main()
