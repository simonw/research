"""Benchmark configuration: URLs, cookies, patterns, paths, ground truth."""

import re
from pathlib import Path
from typing import Any, Dict, List

BASE = Path(__file__).resolve().parents[1]
RUNS_DIR = BASE / "runs"
AB_SOCKET_DIR = Path("/tmp/ab-bench")
RUNTIME_DIR = Path("/tmp/ab-runtime")
CAMOUFOX_BIN = Path.home() / ".cache" / "camoufox" / "camoufox-bin"
PROFILES = BASE / ".profiles"
COOKIES_DIR = BASE / "cookies"
CONTROL_PAGE_DIR = BASE / "scripts" / "control_page"

for d in (RUNS_DIR, AB_SOCKET_DIR, RUNTIME_DIR, PROFILES):
    d.mkdir(parents=True, exist_ok=True)

# ── Target sites ──────────────────────────────────────────────────────────
URLS: Dict[str, Dict[str, Any]] = {
    "x": {
        "page_type": "post",
        "url": "https://x.com/jack/status/20",
        "expected": ["post_text", "author_handle", "timestamp", "canonical_url"],
        "warmup_url": "https://x.com",
    },
    "reddit": {
        "page_type": "post",
        "url": "https://www.reddit.com/r/Python/comments/g53lxf/lad_wrote_a_python_script_to_download_alexa_voice/",
        "expected": ["post_title", "subreddit", "author", "canonical_url"],
        "warmup_url": "https://www.reddit.com",
    },
    "linkedin": {
        "page_type": "company",
        "url": "https://www.linkedin.com/company/microsoft/",
        "expected": ["title_or_company", "location", "page_url", "key_metadata"],
        "warmup_url": "https://www.linkedin.com",
    },
    "instagram": {
        "page_type": "profile",
        "url": "https://www.instagram.com/instagram/",
        "expected": ["username", "canonical_url"],
        "warmup_url": "https://www.instagram.com",
    },
    "control_example": {
        "page_type": "control",
        "url": "https://example.com",
        "expected": ["title"],
        "warmup_url": None,
    },
}

# ── Control / baseline sites ─────────────────────────────────────────────
CONTROL_SITES: Dict[str, Dict[str, Any]] = {
    "control_local": {
        "page_type": "control",
        "url": "http://localhost:__PORT__/",  # port replaced at runtime
        "expected": ["post_text", "author_handle", "timestamp", "canonical_url"],
    },
    "control_example": {
        "page_type": "control",
        "url": "https://example.com",
        "expected": ["title"],
    },
    "control_httpbin": {
        "page_type": "headers",
        "url": "https://httpbin.org/headers",
        "expected": ["user_agent"],
    },
}

# ── Fingerprint detection sites ──────────────────────────────────────────
FINGERPRINT_SITES: Dict[str, Dict[str, Any]] = {
    "sannysoft": {
        "url": "https://bot.sannysoft.com/",
        "measures": "WebDriver flag, navigator props, Chrome automation indicators",
    },
    "creepjs": {
        "url": "https://abrahamjuliot.github.io/creepjs/",
        "measures": "Canvas/WebGL fingerprint, audio fingerprint, font enumeration",
    },
    "pixelscan": {
        "url": "https://pixelscan.net/",
        "measures": "Combined fingerprint consistency score",
    },
}

# ── Cookies (loaded from cookies/<site>.txt in Netscape format) ───────────
def _load_cookies() -> Dict[str, str]:
    cookies: Dict[str, str] = {}
    if not COOKIES_DIR.is_dir():
        return cookies
    for f in COOKIES_DIR.glob("*.txt"):
        cookies[f.stem] = f.read_text(errors="ignore").strip()
    return cookies


COOKIES_RAW: Dict[str, str] = _load_cookies()

# ── Block / failure detection patterns ────────────────────────────────────
BLOCK_PAT = re.compile(
    r"captcha|challenge|suspicious|unusual traffic|verify.{0,20}(human|robot|identity)"
    r"|access denied|temporarily unavailable|just a moment"
    r"|cloudflare|attention required"
    r"|shreddit-forbidden",
    re.I,
)

# Reddit-specific content-unavailable pattern (deleted/removed posts, not anti-bot)
FORBIDDEN_PAT = re.compile(r"shreddit-forbidden", re.I)

# Soft-block indicators (page loaded but content degraded)
SOFT_BLOCK_INDICATORS: Dict[str, Dict[str, Any]] = {
    "x": {"min_page_size": 10000, "required_elements": ['"text":', "@"]},
    "reddit": {"min_page_size": 15000, "required_elements": ["<shreddit-", "r/Python"]},
    "linkedin": {"min_page_size": 20000, "required_elements": ["Microsoft", "employees"]},
    "instagram": {"min_page_size": 10000, "required_elements": ["instagram"]},
    "control_example": {"min_page_size": 100, "required_elements": ["Example Domain"]},
}

# Login/redirect block patterns
LOGIN_REDIRECT_PAT = re.compile(
    r"/login|/challenge|/consent|/accounts/login|/checkpoint|/authenticate",
    re.I,
)

SETUP_PATTERNS = [
    (re.compile(r"Socket directory .* is not writable", re.I), "setup", "socket-dir-permission", "runtime"),
    (re.compile(r"Session name .* is too long", re.I), "setup", "session-name-too-long", "runtime"),
    (re.compile(r"Executable doesn't exist at .*ms-playwright", re.I), "setup", "missing-browser-binary", "preflight"),
    (re.compile(r"Please run the following command to download new browsers", re.I), "setup", "missing-browser-binary", "preflight"),
    (re.compile(r"No module named|ModuleNotFoundError|ImportError", re.I), "setup", "missing-python-module", "preflight"),
]

STARTUP_PATTERNS = [
    (re.compile(r"Daemon failed to start", re.I), "startup", "daemon-start-failed", "browser-startup"),
    (re.compile(r"Firefox is already running, but is not responding", re.I), "startup", "profile-lock", "browser-startup"),
    (re.compile(r"Target page, context or browser has been closed", re.I), "startup", "browser-closed", "browser-startup"),
]

TIMEOUT_PATTERNS = [
    (re.compile(r"Timeout \d+ms exceeded|timeout", re.I), "timeout", "navigation-timeout", "navigation"),
]

# ── Ground truth for correctness validation ──────────────────────────────
GROUND_TRUTH: Dict[str, Dict[str, Any]] = {
    "x": {
        "post_text_contains": "just setting up my twttr",
        "author_handle": "jack",
        "canonical_url_contains": "x.com/jack/status/20",
    },
    "reddit": {
        "post_title_contains": "python script",
        "subreddit": "Python",
        "author": "iEslam",
        "canonical_url_contains": "reddit.com/r/Python/comments/g53lxf",
    },
    "linkedin": {
        "title_or_company_contains": "Microsoft",
        "page_url_contains": "linkedin.com/company/microsoft",
    },
    "instagram": {
        "username_contains": "instagram",
        "canonical_url_contains": "instagram.com/instagram",
    },
    "control_local": {
        "post_text_contains": "benchmark control page",
        "author_handle": "benchmark-bot",
        "canonical_url_contains": "localhost",
    },
    "control_example": {
        "title_contains": "Example Domain",
    },
    "control_httpbin": {
        "user_agent_present": True,
    },
}
