"""Benchmark configuration: URLs, cookies, patterns, paths, ground truth."""

import re
from pathlib import Path
from typing import Any, Dict, List

BASE = Path(__file__).resolve().parents[1]
ART = BASE / "artifacts"
RES = BASE / "results"
AB_SOCKET_DIR = Path("/tmp/ab-bench")
RUNTIME_DIR = Path("/tmp/ab-runtime")
CAMOUFOX_BIN = Path.home() / ".cache" / "camoufox" / "camoufox-bin"
PROFILES = BASE / ".profiles"
CONTROL_PAGE_DIR = BASE / "scripts" / "control_page"

for d in (ART, RES, AB_SOCKET_DIR, RUNTIME_DIR, PROFILES):
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
        "url": "https://www.reddit.com/r/Python/comments/10wxbk8/whats_everyone_working_on_this_week/",
        "expected": ["post_title", "post_body", "subreddit", "author", "timestamp", "canonical_url"],
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
        "expected": ["username", "timestamp", "canonical_url"],
        "warmup_url": "https://www.instagram.com",
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

# ── Cookies (Netscape format) ────────────────────────────────────────────
COOKIES_RAW: Dict[str, str] = {
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

# ── Block / failure detection patterns ────────────────────────────────────
BLOCK_PAT = re.compile(
    r"captcha|challenge|suspicious|unusual traffic|verify.{0,20}(human|robot|identity)"
    r"|access denied|temporarily unavailable|just a moment"
    r"|cloudflare|attention required",
    re.I,
)

# Soft-block indicators (page loaded but content degraded)
SOFT_BLOCK_INDICATORS: Dict[str, Dict[str, Any]] = {
    "x": {"min_page_size": 10000, "required_elements": ['"text":', "@"]},
    "reddit": {"min_page_size": 15000, "required_elements": ["<shreddit-", "r/Python"]},
    "linkedin": {"min_page_size": 20000, "required_elements": ["Microsoft", "employees"]},
    "instagram": {"min_page_size": 10000, "required_elements": ["instagram"]},
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
        "subreddit": "Python",
        "post_title_contains": "working on this week",
        "canonical_url_contains": "reddit.com/r/Python",
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
