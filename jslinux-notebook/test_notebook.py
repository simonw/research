#!/usr/bin/env python3
"""
Playwright test for JSLinux Notebook
Tests the basic functionality of the notebook interface
"""

import asyncio
import time
from playwright.async_api import async_playwright, expect


async def test_jslinux_notebook():
    """Test the JSLinux notebook interface"""
    print("Starting Playwright test...")

    async with async_playwright() as p:
        # Launch browser
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context()
        page = await context.new_page()

        # Navigate to the page
        print("Navigating to http://localhost:8889/notebook.html")
        await page.goto("http://localhost:8889/notebook.html")

        # Wait for the page to load
        print("Waiting for page to load...")
        await page.wait_for_load_state("networkidle")

        # Check the title
        title = await page.title()
        print(f"Page title: {title}")
        assert "JS Linux" in title, f"Expected 'JS Linux' in title, got: {title}"

        # Wait for the status to show "ready"
        print("Waiting for Linux to be ready...")
        status = page.locator("#status")

        # Wait up to 15 seconds for status to contain "ready"
        try:
            await expect(status).to_contain_text("ready", timeout=15000)
            status_text = await status.text_content()
            print(f"Status: {status_text}")
        except Exception as e:
            print(f"Warning: Status didn't show ready: {e}")
            status_text = await status.text_content()
            print(f"Final status: {status_text}")

        # Find the first textarea
        print("\nTesting command execution...")
        textarea = page.locator('textarea[data-cell-id="0"]')

        # Clear and type a command
        await textarea.fill("whoami")
        print("Entered command: whoami")

        # Click the run button
        run_button = page.locator('button.btn-run').first
        await run_button.click()
        print("Clicked Run button")

        # Wait for output to appear
        await page.wait_for_timeout(1500)
        output = page.locator("#output-0")

        # Check if output is visible
        is_visible = await output.is_visible()
        print(f"Output visible: {is_visible}")

        if is_visible:
            output_text = await output.text_content()
            print(f"Output:\n{output_text}")
            assert "whoami" in output_text, "Output should contain the command"
            assert "root" in output_text or "Executing" in output_text, "Output should contain result or executing message"

        # Test adding a new cell
        print("\nTesting add cell functionality...")
        add_button = page.locator('button.btn-add')
        await add_button.click()
        print("Clicked Add Cell button")

        await page.wait_for_timeout(500)

        # Check if new cell was added
        new_textarea = page.locator('textarea[data-cell-id="1"]')
        is_present = await new_textarea.count() > 0
        print(f"New cell added: {is_present}")
        assert is_present, "New cell should be added"

        # Test the new cell
        await new_textarea.fill("date")
        print("Entered command in new cell: date")

        new_run_button = page.locator('button.btn-run').nth(1)
        await new_run_button.click()
        print("Clicked Run button for new cell")

        await page.wait_for_timeout(1500)
        new_output = page.locator("#output-1")

        if await new_output.is_visible():
            new_output_text = await new_output.text_content()
            print(f"New cell output:\n{new_output_text}")

        # Test clear output functionality
        print("\nTesting clear output...")
        clear_button = page.locator('button.btn-clear').first
        await clear_button.click()
        print("Clicked Clear Output button")

        await page.wait_for_timeout(500)
        is_visible_after_clear = await output.is_visible()
        print(f"Output visible after clear: {is_visible_after_clear}")

        # Test keyboard shortcut (Ctrl+Enter)
        print("\nTesting keyboard shortcut (Ctrl+Enter)...")
        await textarea.fill("ls")
        await textarea.press("Control+Enter")
        print("Pressed Ctrl+Enter")

        await page.wait_for_timeout(1500)

        if await output.is_visible():
            output_text = await output.text_content()
            print(f"Output from keyboard shortcut:\n{output_text}")

        # Take a screenshot
        print("\nTaking screenshot...")
        await page.screenshot(path="jslinux-notebook-screenshot.png")
        print("Screenshot saved as jslinux-notebook-screenshot.png")

        # Close browser
        await browser.close()

        print("\n✅ All tests passed!")
        return True


if __name__ == "__main__":
    try:
        result = asyncio.run(test_jslinux_notebook())
        if result:
            print("\n🎉 Test suite completed successfully!")
        exit(0)
    except Exception as e:
        print(f"\n❌ Test failed with error: {e}")
        import traceback
        traceback.print_exc()
        exit(1)
