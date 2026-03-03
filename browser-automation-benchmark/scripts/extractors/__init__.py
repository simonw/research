"""Layered extraction: JSON-LD → Open Graph → regex fallback."""

from typing import Any, Dict

from .json_ld import extract_json_ld
from .opengraph import extract_opengraph
from .regex_fallback import extract_regex
from config import GROUND_TRUTH


def extract(site: str, text: str, url: str) -> Dict[str, str]:
    """Extract structured data using a layered approach.

    1. JSON-LD (structured, tool-agnostic)
    2. Open Graph meta tags (fallback)
    3. Site-specific regex (last resort)

    Returns dict of field_name -> value for the site.
    """
    # Layer 1: JSON-LD
    fields = extract_json_ld(site, text, url)

    # Layer 2: Open Graph (fill gaps)
    og = extract_opengraph(site, text, url)
    for k, v in og.items():
        if not fields.get(k):
            fields[k] = v

    # Layer 3: Regex fallback (fill remaining gaps)
    rx = extract_regex(site, text, url)
    for k, v in rx.items():
        if not fields.get(k):
            fields[k] = v

    return fields


def validate_ground_truth(site: str, extracted: Dict[str, str]) -> Dict[str, Any]:
    """Validate extracted values against known ground truth.

    Returns:
        {
            "checks": {field: {"expected": ..., "actual": ..., "pass": bool}},
            "correctness_pct": float,
        }
    """
    truth = GROUND_TRUTH.get(site, {})
    if not truth:
        return {"checks": {}, "correctness_pct": None}

    checks = {}
    for key, expected_val in truth.items():
        if key.endswith("_contains"):
            field = key[: -len("_contains")]
            actual = extracted.get(field, "")
            passed = expected_val.lower() in actual.lower() if actual else False
            checks[key] = {"expected": expected_val, "actual": actual[:200], "pass": passed}
        elif key.endswith("_present"):
            field = key[: -len("_present")]
            actual = extracted.get(field, "")
            passed = bool(actual)
            checks[key] = {"expected": "non-empty", "actual": bool(actual), "pass": passed}
        else:
            actual = extracted.get(key, "")
            passed = expected_val.lower() == actual.lower() if actual else False
            checks[key] = {"expected": expected_val, "actual": actual[:200], "pass": passed}

    total = len(checks)
    passed = sum(1 for c in checks.values() if c["pass"])
    return {
        "checks": checks,
        "correctness_pct": round(100 * passed / total, 2) if total else None,
    }
