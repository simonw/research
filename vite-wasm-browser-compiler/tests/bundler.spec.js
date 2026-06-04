// @ts-check
const { test, expect } = require('@playwright/test');

test.describe('Browser-Based Single File Bundler', () => {

    test.beforeEach(async ({ page }) => {
        // Go to the bundler page
        await page.goto('/index.html');

        // Wait for esbuild to initialize (check the log)
        await expect(page.locator('#logOutput')).toContainText('esbuild-wasm initialized successfully', { timeout: 60000 });
    });

    test('page loads correctly', async ({ page }) => {
        // Check title
        await expect(page).toHaveTitle('Browser-Based Single File Bundler');

        // Check main elements are present
        await expect(page.locator('h1')).toContainText('Browser-Based Single File Bundler');
        await expect(page.locator('#urlInput')).toBeVisible();
        await expect(page.locator('#bundleBtn')).toBeVisible();
        await expect(page.locator('#outputArea')).toBeVisible();
    });

    test('esbuild-wasm initializes', async ({ page }) => {
        // Check log shows esbuild initialized
        const logOutput = page.locator('#logOutput');
        await expect(logOutput).toContainText('Initializing esbuild-wasm');
        await expect(logOutput).toContainText('esbuild-wasm initialized successfully');
    });

    test('bundles simple HTML page with CSS and JS', async ({ page }) => {
        // Enter the test page URL
        await page.fill('#urlInput', 'http://localhost:3000/test-pages/simple-app/index.html');

        // Click bundle button
        await page.click('#bundleBtn');

        // Wait for bundling to complete
        await expect(page.locator('#status')).toContainText('Bundling complete', { timeout: 60000 });

        // Check that output is not empty
        const outputArea = page.locator('#outputArea');
        const output = await outputArea.inputValue();

        expect(output.length).toBeGreaterThan(0);
        expect(output).toContain('<!DOCTYPE html>');

        // Check that CSS was inlined (should contain <style> tag with our styles)
        expect(output).toContain('<style>');
        expect(output).toContain('font-family');
        expect(output).toContain('linear-gradient');

        // Check that JS was inlined (should contain <script> tag with our code)
        expect(output).toContain('Count:');
        expect(output).toContain('addEventListener');

        // Check that external references are removed
        expect(output).not.toContain('href="styles.css"');
        expect(output).not.toContain('src="main.js"');
    });

    test('stats are displayed after bundling', async ({ page }) => {
        // Enter the test page URL
        await page.fill('#urlInput', 'http://localhost:3000/test-pages/simple-app/index.html');

        // Click bundle button
        await page.click('#bundleBtn');

        // Wait for bundling to complete
        await expect(page.locator('#status')).toContainText('Bundling complete', { timeout: 60000 });

        // Check stats are displayed
        const stats = page.locator('#stats');
        await expect(stats).toBeVisible();

        // Check individual stats
        await expect(page.locator('#originalSize')).not.toHaveText('-');
        await expect(page.locator('#bundledSize')).not.toHaveText('-');
        await expect(page.locator('#resourceCount')).not.toHaveText('-');
        await expect(page.locator('#timeTaken')).not.toHaveText('-');
    });

    test('copy button works', async ({ page }) => {
        // Enter the test page URL
        await page.fill('#urlInput', 'http://localhost:3000/test-pages/simple-app/index.html');

        // Click bundle button
        await page.click('#bundleBtn');

        // Wait for bundling to complete
        await expect(page.locator('#status')).toContainText('Bundling complete', { timeout: 60000 });

        // Copy button should be enabled
        await expect(page.locator('#copyBtn')).toBeEnabled();

        // Grant clipboard permissions
        await page.context().grantPermissions(['clipboard-write', 'clipboard-read']);

        // Click copy button
        await page.click('#copyBtn');

        // Check status message
        await expect(page.locator('#status')).toContainText('Copied to clipboard');
    });

    test('preview tab shows bundled content', async ({ page }) => {
        // Enter the test page URL
        await page.fill('#urlInput', 'http://localhost:3000/test-pages/simple-app/index.html');

        // Click bundle button
        await page.click('#bundleBtn');

        // Wait for bundling to complete
        await expect(page.locator('#status')).toContainText('Bundling complete', { timeout: 60000 });

        // Switch to preview tab
        await page.click('text=Preview');

        // Check that preview iframe is visible
        const previewFrame = page.locator('#previewFrame');
        await expect(previewFrame).toBeVisible();

        // Check that the iframe has content loaded
        const frame = page.frameLocator('#previewFrame');
        await expect(frame.locator('h1')).toContainText('Hello from Simple App');
    });

    test('bundled output is self-contained and works', async ({ page }) => {
        // Enter the test page URL
        await page.fill('#urlInput', 'http://localhost:3000/test-pages/simple-app/index.html');

        // Click bundle button
        await page.click('#bundleBtn');

        // Wait for bundling to complete
        await expect(page.locator('#status')).toContainText('Bundling complete', { timeout: 60000 });

        // Get the bundled output
        const outputArea = page.locator('#outputArea');
        const bundledHtml = await outputArea.inputValue();

        // Verify no external resource references
        expect(bundledHtml).not.toMatch(/src=["'][^"']*\.js["']/);
        expect(bundledHtml).not.toMatch(/href=["'][^"']*\.css["']/);

        // Switch to preview tab
        await page.click('text=Preview');

        // In the preview iframe, check the app works
        const frame = page.frameLocator('#previewFrame');

        // Check initial state
        await expect(frame.locator('#message')).toContainText('JavaScript is working');

        // Click the counter button
        await frame.locator('#counter-btn').click();
        await expect(frame.locator('#counter-btn')).toContainText('Count: 1');

        // Click again
        await frame.locator('#counter-btn').click();
        await expect(frame.locator('#counter-btn')).toContainText('Count: 2');
    });

    test('handles errors gracefully for invalid URLs', async ({ page }) => {
        // Enter an invalid URL
        await page.fill('#urlInput', 'http://localhost:3000/nonexistent.html');

        // Click bundle button
        await page.click('#bundleBtn');

        // Should show error status
        await expect(page.locator('#status')).toContainText('Error', { timeout: 30000 });

        // Log should show the error
        await expect(page.locator('#logOutput')).toContainText('failed', { timeout: 10000 });
    });

    test('shows warning when no URL is entered', async ({ page }) => {
        // Make sure URL input is empty
        await page.fill('#urlInput', '');

        // Click bundle button
        await page.click('#bundleBtn');

        // Should show error about empty URL
        await expect(page.locator('#status')).toContainText('Please enter a URL');
    });

});
