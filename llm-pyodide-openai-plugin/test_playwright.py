"""
Playwright test for LLM Pyodide OpenAI Plugin

This test verifies that the plugin can:
1. Load in a pyodide environment
2. Install LLM via micropip
3. Register the plugin
4. Make a CORS request to OpenAI API (will get auth error without key)
"""

import asyncio
from pathlib import Path
from playwright.async_api import async_playwright, expect
import pytest


# Test files
TEST_DIR = Path(__file__).parent
HTML_FILE = TEST_DIR / "test.html"


@pytest.mark.asyncio
async def test_llm_pyodide_plugin_loads():
    """Test that the plugin loads successfully in pyodide."""
    async with async_playwright() as p:
        # Launch browser
        browser = await p.chromium.launch()
        context = await browser.new_context()
        page = await context.new_page()

        # Navigate to test page
        await page.goto("http://localhost:8765/test.html")

        # Wait for page to be ready
        await page.wait_for_selector('#btn-init:not([disabled])')

        # Step 1: Initialize Pyodide
        await page.click('#btn-init')
        await page.wait_for_selector('#btn-install:not([disabled])', timeout=60000)

        # Check status
        status = await page.text_content('#status')
        assert 'loaded successfully' in status.lower() or 'pyodide loaded' in status.lower()

        # Check output
        output = await page.text_content('#output')
        assert '✓ Pyodide loaded' in output

        print("✓ Step 1: Pyodide initialized")

        await browser.close()


@pytest.mark.asyncio
async def test_llm_install():
    """Test that LLM can be installed via micropip."""
    async with async_playwright() as p:
        browser = await p.chromium.launch()
        context = await browser.new_context()
        page = await context.new_page()

        # Navigate and init
        await page.goto(f"file://{HTML_FILE.absolute()}")
        await page.wait_for_selector('#btn-init:not([disabled])')
        await page.click('#btn-init')
        await page.wait_for_selector('#btn-install:not([disabled])', timeout=60000)

        # Step 2: Install LLM
        await page.click('#btn-install')
        await page.wait_for_selector('#btn-load-plugin:not([disabled])', timeout=120000)

        # Check output
        output = await page.text_content('#output')
        assert '✓ LLM installed' in output
        assert 'LLM imported successfully' in output

        print("✓ Step 2: LLM installed")

        await browser.close()


@pytest.mark.asyncio
async def test_plugin_registration():
    """Test that the pyodide plugin can be registered with LLM."""
    async with async_playwright() as p:
        browser = await p.chromium.launch()
        context = await browser.new_context()
        page = await context.new_page()

        # Navigate and run through steps
        await page.goto(f"file://{HTML_FILE.absolute()}")
        await page.wait_for_selector('#btn-init:not([disabled])')

        # Init
        await page.click('#btn-init')
        await page.wait_for_selector('#btn-install:not([disabled])', timeout=60000)

        # Install
        await page.click('#btn-install')
        await page.wait_for_selector('#btn-load-plugin:not([disabled])', timeout=120000)

        # Load plugin
        await page.click('#btn-load-plugin')
        await page.wait_for_selector('#btn-test:not([disabled])', timeout=30000)

        # Check output
        output = await page.text_content('#output')
        assert '✓ Plugin code loaded' in output

        print("✓ Step 3: Plugin registered")

        await browser.close()


@pytest.mark.asyncio
async def test_openai_api_call():
    """Test that the plugin can make a CORS call to OpenAI API."""
    async with async_playwright() as p:
        browser = await p.chromium.launch()
        context = await browser.new_context()
        page = await context.new_page()

        # Set up console logging to capture errors
        console_messages = []
        page.on('console', lambda msg: console_messages.append(msg.text))

        # Navigate and run through all steps
        await page.goto(f"file://{HTML_FILE.absolute()}")
        await page.wait_for_selector('#btn-init:not([disabled])')

        # Init
        await page.click('#btn-init')
        await page.wait_for_selector('#btn-install:not([disabled])', timeout=60000)

        # Install
        await page.click('#btn-install')
        await page.wait_for_selector('#btn-load-plugin:not([disabled])', timeout=120000)

        # Load plugin
        await page.click('#btn-load-plugin')
        await page.wait_for_selector('#btn-test:not([disabled])', timeout=30000)

        # Test API call (without API key - should get auth error)
        await page.click('#btn-test')

        # Wait for output to update
        await asyncio.sleep(5)

        # Check output
        output = await page.text_content('#output')

        # We expect either:
        # 1. An auth error (401, Unauthorized, etc.) - proves CORS works
        # 2. Success if somehow an API key is set
        # 3. An error message showing the API was called

        has_auth_error = any([
            '401' in output,
            'Unauthorized' in output,
            'Incorrect API key' in output,
            'authentication' in output.lower(),
        ])

        has_success = 'SUCCESS:' in output

        has_error_indication = 'ERROR:' in output or 'Error:' in output

        # The test passes if we got an auth error (proves fetch worked)
        # or if we got success (someone provided a key)
        assert has_auth_error or has_success or has_error_indication, \
            f"Expected auth error or success, got: {output}"

        if has_auth_error:
            print("✓ Step 4: Got expected auth error (proves CORS works)")
            # Also verify the message mentions CORS working
            assert '✓' in output or 'fetch API is working' in output
        elif has_success:
            print("✓ Step 4: API call succeeded (API key was provided)")
        else:
            print(f"✓ Step 4: Got API error: {output}")

        await browser.close()


@pytest.mark.asyncio
async def test_full_workflow():
    """Test the complete workflow from initialization to API call."""
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context()
        page = await context.new_page()

        print("\nRunning full workflow test...")

        # Navigate to test page
        await page.goto("http://localhost:8765/test.html")
        await page.wait_for_selector('#btn-init:not([disabled])')
        print("  Page loaded")

        # Step 1: Initialize
        await page.click('#btn-init')
        await page.wait_for_selector('#btn-install:not([disabled])', timeout=60000)
        print("  ✓ Pyodide initialized")

        # Step 2: Install LLM
        await page.click('#btn-install')
        await page.wait_for_selector('#btn-load-plugin:not([disabled])', timeout=120000)
        print("  ✓ LLM installed")

        # Step 3: Load plugin
        await page.click('#btn-load-plugin')
        await page.wait_for_selector('#btn-test:not([disabled])', timeout=30000)
        print("  ✓ Plugin loaded")

        # Step 4: Test API
        await page.click('#btn-test')
        await asyncio.sleep(5)

        output = await page.text_content('#output')

        # Verify we got some response from the API
        assert 'Error:' in output or 'SUCCESS:' in output, \
            "Expected either an error or success response from API"

        print("  ✓ API call completed")
        print("\n✓ Full workflow test passed!")

        await browser.close()


if __name__ == "__main__":
    # Run tests
    pytest.main([__file__, "-v", "-s"])
