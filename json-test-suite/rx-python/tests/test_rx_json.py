"""
Test suite for the Python rx implementation, driven by the shared JSON test file.
"""

import json
import math
from pathlib import Path
import sys

import pytest

# Add src to path
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from rx_python.rx import (
    b64_stringify,
    b64_parse,
    to_zigzag,
    from_zigzag,
    split_number,
    stringify,
    parse,
    encode,
    open_buffer,
    UNDEFINED,
)

# Load the JSON test suite
SUITE_PATH = Path(__file__).parent.parent.parent / "rx-tests.json"
with open(SUITE_PATH) as f:
    SUITE = json.load(f)


def decode_value(v):
    """Decode a JSON test value, handling __special wrappers."""
    if isinstance(v, dict):
        if "__special" in v:
            special = v["__special"]
            if special == "NaN":
                return float("nan")
            if special == "Infinity":
                return float("inf")
            if special == "-Infinity":
                return float("-inf")
            if special == "undefined":
                return UNDEFINED
        return {k: decode_value(val) for k, val in v.items()}
    if isinstance(v, list):
        return [decode_value(item) for item in v]
    return v


def deep_equal(actual, expected):
    """Deep compare that handles NaN, UNDEFINED, and special values."""
    if expected is UNDEFINED:
        assert actual is UNDEFINED, f"Expected UNDEFINED, got {actual!r}"
        return

    if isinstance(expected, float) and math.isnan(expected):
        assert isinstance(actual, float) and math.isnan(actual), f"Expected NaN, got {actual!r}"
        return

    if isinstance(expected, dict):
        assert isinstance(actual, dict), f"Expected dict, got {type(actual)}"
        assert sorted(actual.keys()) == sorted(expected.keys()), (
            f"Key mismatch: {sorted(actual.keys())} != {sorted(expected.keys())}"
        )
        for key in expected:
            deep_equal(actual[key], expected[key])
        return

    if isinstance(expected, list):
        assert isinstance(actual, list), f"Expected list, got {type(actual)}"
        assert len(actual) == len(expected), f"Length mismatch: {len(actual)} != {len(expected)}"
        for i, (a, e) in enumerate(zip(actual, expected)):
            deep_equal(a, e)
        return

    if isinstance(expected, float):
        assert actual == pytest.approx(expected, rel=1e-10), f"Expected {expected}, got {actual}"
        return

    assert actual == expected, f"Expected {expected!r}, got {actual!r}"


# ── b64 stringify ──

class TestB64Stringify:
    @pytest.mark.parametrize(
        "input_val,expected",
        [(t["input"], t["expected"]) for t in SUITE["b64_stringify"]["tests"]],
        ids=[f"{t['input']}" for t in SUITE["b64_stringify"]["tests"]],
    )
    def test_b64_stringify(self, input_val, expected):
        assert b64_stringify(input_val) == expected


# ── b64 parse ──

class TestB64Parse:
    @pytest.mark.parametrize(
        "input_val,expected",
        [(t["input"], t["expected"]) for t in SUITE["b64_parse"]["tests"]],
        ids=[f'"{t["input"]}"' for t in SUITE["b64_parse"]["tests"]],
    )
    def test_b64_parse(self, input_val, expected):
        assert b64_parse(input_val) == expected


# ── zigzag encode ──

class TestZigzagEncode:
    @pytest.mark.parametrize(
        "input_val,expected",
        [(t["input"], t["expected"]) for t in SUITE["zigzag_encode"]["tests"]],
        ids=[f"{t['input']}" for t in SUITE["zigzag_encode"]["tests"]],
    )
    def test_zigzag_encode(self, input_val, expected):
        assert to_zigzag(input_val) == expected


# ── zigzag decode ──

class TestZigzagDecode:
    @pytest.mark.parametrize(
        "input_val,expected",
        [(t["input"], t["expected"]) for t in SUITE["zigzag_decode"]["tests"]],
        ids=[f"{t['input']}" for t in SUITE["zigzag_decode"]["tests"]],
    )
    def test_zigzag_decode(self, input_val, expected):
        assert from_zigzag(input_val) == expected


# ── stringify ──

class TestStringify:
    @pytest.mark.parametrize(
        "test_case",
        SUITE["stringify"]["tests"],
        ids=[t["name"] for t in SUITE["stringify"]["tests"]],
    )
    def test_stringify(self, test_case):
        input_val = decode_value(test_case["input"])
        opts = test_case.get("options", {})
        result = stringify(input_val, opts)
        assert result == test_case["expected"]


# ── parse ──

class TestParse:
    @pytest.mark.parametrize(
        "test_case",
        SUITE["parse"]["tests"],
        ids=[t["name"] for t in SUITE["parse"]["tests"]],
    )
    def test_parse(self, test_case):
        result = parse(test_case["input"])
        expected = decode_value(test_case["expected"])
        deep_equal(result, expected)


# ── roundtrip ──

class TestRoundtrip:
    @pytest.mark.parametrize(
        "test_case",
        SUITE["roundtrip"]["tests"],
        ids=[t["name"] for t in SUITE["roundtrip"]["tests"]],
    )
    def test_roundtrip(self, test_case):
        value = decode_value(test_case["value"])
        opts = test_case.get("options", {})
        encoded = encode(value, opts)
        refs = opts.get("refs")
        result = open_buffer(encoded, refs)
        deep_equal(result, value)


# ── split number ──

class TestSplitNumber:
    @pytest.mark.parametrize(
        "test_case",
        SUITE["split_number"]["tests"],
        ids=[f"{t['input']}" for t in SUITE["split_number"]["tests"]],
    )
    def test_split_number(self, test_case):
        base, exp = split_number(test_case["input"])
        assert base == test_case["base"]
        assert exp == test_case["exponent"]
