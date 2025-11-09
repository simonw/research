"""
Playwright test for the Linux VM JS Library
Tests the demo page to ensure the VM boots and can execute commands
"""

import subprocess
import time
import signal
import os
from playwright.sync_api import sync_playwright, expect


def test_linux_vm():
    """Test that the Linux VM can boot and execute commands"""

    # Get the directory where this script is located
    script_dir = os.path.dirname(os.path.abspath(__file__))

    # Start custom HTTP server with proper headers for v86
    print("Starting HTTP server on port 8000...")
    server_process = subprocess.Popen(
        ["python3", os.path.join(script_dir, "server.py")],
        cwd=script_dir,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        preexec_fn=os.setsid  # Create new process group for clean shutdown
    )

    # Give server time to start
    time.sleep(2)

    try:
        with sync_playwright() as p:
            # Launch browser (headless mode)
            print("Launching browser...")
            browser = p.chromium.launch(headless=True)
            context = browser.new_context()
            page = context.new_page()

            # Enable console logging
            page.on("console", lambda msg: print(f"Browser console: {msg.text}"))

            # Navigate to demo page
            print("Loading demo page...")
            page.goto("http://localhost:8000/demo.html")

            # Verify page loaded
            expect(page.locator("h1")).to_contain_text("Linux VM JS Library Demo")
            print("✓ Page loaded successfully")

            # Click Start VM button
            print("\nStarting VM (this will take 30-60 seconds)...")
            start_btn = page.locator("#startBtn")
            start_btn.click()

            # Wait for VM to boot (increase timeout for VM boot)
            # The status should change from "Creating VM instance..." to "VM is ready!"
            print("Waiting for VM to boot...")
            page.wait_for_function(
                "document.getElementById('status').textContent.includes('VM is ready')",
                timeout=90000  # 90 seconds for boot
            )
            print("✓ VM booted successfully")

            # Verify test button is enabled
            test_btn = page.locator("#testBtn")
            expect(test_btn).to_be_enabled()

            # Click Run Test Command button
            print("\nExecuting test command...")
            test_btn.click()

            # Wait for command execution to complete
            page.wait_for_function(
                "document.getElementById('status').textContent.includes('All tests complete')",
                timeout=30000  # 30 seconds for command execution
            )
            print("✓ Test command executed")

            # Verify output contains expected results
            output = page.locator("#output").text_content()
            print("\n--- VM Output ---")
            print(output)
            print("--- End Output ---\n")

            # Check that output contains our test file
            assert "hello.txt" in output, "Output should contain 'hello.txt'"
            print("✓ Output contains 'hello.txt'")

            # Check that the file content was read correctly
            assert "hello" in output, "Output should contain 'hello' from cat command"
            print("✓ Output contains 'hello' from file content")

            print("\n✓ All tests passed!")

            # Shutdown VM
            print("\nShutting down VM...")
            shutdown_btn = page.locator("#shutdownBtn")
            shutdown_btn.click()
            time.sleep(1)

            browser.close()

    finally:
        # Cleanup: Stop HTTP server
        print("\nStopping HTTP server...")
        os.killpg(os.getpgid(server_process.pid), signal.SIGTERM)
        server_process.wait()
        print("✓ Server stopped")


if __name__ == "__main__":
    test_linux_vm()
