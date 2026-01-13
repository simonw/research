#!/usr/bin/env python3
"""
Test runner for string redaction library.
Reads test cases from YAML and runs them against the implementation.
"""

import yaml
import sys
from typing import List, Dict, Any
from redactor import detect_secrets


def load_tests(yaml_path: str = "tests.yaml") -> List[Dict[str, Any]]:
    """Load test cases from YAML file."""
    with open(yaml_path, "r") as f:
        data = yaml.safe_load(f)
    return data.get("test_cases", [])


def run_test(test_case: Dict[str, Any]) -> tuple[bool, str]:
    """
    Run a single test case.
    Returns (passed, message).
    """
    name = test_case["name"]
    input_text = test_case["input"]
    expected = test_case["expected"]

    try:
        result = detect_secrets(input_text)
    except Exception as e:
        return False, f"Exception: {e}"

    # Convert result to comparable format
    result_normalized = [
        {"value": r["value"], "start": r["start"], "end": r["end"]}
        for r in result
    ]

    # Normalize expected (ensure it's a list)
    expected_normalized = expected if expected else []

    if result_normalized == expected_normalized:
        return True, "PASS"
    else:
        return False, f"FAIL\n  Expected: {expected_normalized}\n  Got:      {result_normalized}"


def main():
    """Run all tests and report results."""
    tests = load_tests()

    passed = 0
    failed = 0

    print("=" * 60)
    print("String Redaction Library - Test Suite")
    print("=" * 60)
    print()

    for test in tests:
        name = test["name"]
        description = test.get("description", "")

        success, message = run_test(test)

        if success:
            passed += 1
            print(f"  PASS: {name}")
        else:
            failed += 1
            print(f"  FAIL: {name}")
            print(f"        {description}")
            print(f"        {message}")

    print()
    print("=" * 60)
    print(f"Results: {passed} passed, {failed} failed, {len(tests)} total")
    print("=" * 60)

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
