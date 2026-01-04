#!/usr/bin/env python3
"""
Playwright test for quickjs.html JavaScript executor.
"""

import subprocess
import time
import signal
import sys
from playwright.sync_api import sync_playwright


def test_quickjs_html():
    """Test the QuickJS HTML executor."""

    # Start a simple HTTP server
    server_process = subprocess.Popen(
        ["python", "-m", "http.server", "8765", "--directory", "/tmp"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    # Give the server time to start
    time.sleep(2)

    try:
        with sync_playwright() as p:
            browser = p.chromium.launch(headless=True)
            page = browser.new_page()

            # Enable console logging for debugging
            page.on("console", lambda msg: print(f"BROWSER: {msg.text}"))
            page.on("pageerror", lambda err: print(f"PAGE ERROR: {err}"))

            print("Navigating to quickjs.html...")
            page.goto("http://localhost:8765/quickjs.html", timeout=60000)

            # Wait for the QuickJS runtime to initialize (longer timeout)
            print("Waiting for QuickJS to initialize...")
            page.wait_for_selector("#run-btn:not([disabled])", timeout=60000)
            print("QuickJS initialized!")

            # Test 1: Hello World
            print("\n=== Test 1: Hello World ===")
            page.fill("#code-input", "console.log('Hello from QuickJS!');")
            page.click("#run-btn")
            page.wait_for_selector("#output-section.visible", timeout=10000)
            output = page.text_content("#output")
            print(f"Output: {output}")
            assert "Hello from QuickJS!" in output, f"Expected 'Hello from QuickJS!' in output, got: {output}"
            print("Test 1 PASSED!")

            # Test 2: Fibonacci
            print("\n=== Test 2: Fibonacci ===")
            fibonacci_code = """
function fibonacci(n) {
    const seq = [0, 1];
    for (let i = 2; i < n; i++) {
        seq.push(seq[i-1] + seq[i-2]);
    }
    return seq;
}
console.log(fibonacci(10).join(', '));
"""
            page.fill("#code-input", fibonacci_code)
            page.click("#run-btn")
            time.sleep(1)
            output = page.text_content("#output")
            print(f"Output: {output}")
            assert "0, 1, 1, 2, 3, 5, 8, 13, 21, 34" in output, f"Fibonacci test failed: {output}"
            print("Test 2 PASSED!")

            # Test 3: Array operations
            print("\n=== Test 3: Array Operations ===")
            array_code = """
const nums = [1, 2, 3, 4, 5];
const doubled = nums.map(n => n * 2);
console.log('Doubled:', JSON.stringify(doubled));
"""
            page.fill("#code-input", array_code)
            page.click("#run-btn")
            time.sleep(1)
            output = page.text_content("#output")
            print(f"Output: {output}")
            assert "[2,4,6,8,10]" in output, f"Array test failed: {output}"
            print("Test 3 PASSED!")

            # Test 4: JSON operations
            print("\n=== Test 4: JSON Operations ===")
            json_code = """
const obj = { name: 'test', value: 42 };
console.log(JSON.stringify(obj));
"""
            page.fill("#code-input", json_code)
            page.click("#run-btn")
            time.sleep(1)
            output = page.text_content("#output")
            print(f"Output: {output}")
            assert '"name":"test"' in output and '"value":42' in output, f"JSON test failed: {output}"
            print("Test 4 PASSED!")

            browser.close()

        print("\n" + "=" * 50)
        print("All QuickJS tests PASSED!")
        print("=" * 50)
        return True

    except Exception as e:
        print(f"\nTest failed with error: {e}", file=sys.stderr)
        return False

    finally:
        # Stop the HTTP server
        server_process.send_signal(signal.SIGTERM)
        server_process.wait()


if __name__ == "__main__":
    success = test_quickjs_html()
    sys.exit(0 if success else 1)
