// @ts-check
const { test, expect } = require('@playwright/test');

test.describe('Vite Single File Bundler (WebContainer)', () => {

    test.beforeEach(async ({ page }) => {
        // Go to the WebContainer bundler page (requires COI headers)
        await page.goto('/index-webcontainer.html');

        // Wait for WebContainer to boot (this can take a while)
        await expect(page.locator('#logOutput')).toContainText('WebContainer booted successfully', { timeout: 120000 });
    });

    test('page loads and WebContainer boots', async ({ page }) => {
        // Check title
        await expect(page).toHaveTitle(/Vite Single File Bundler/);

        // Check main elements are present
        await expect(page.locator('h1')).toContainText('Vite Single File Bundler');
        await expect(page.locator('#urlInput')).toBeVisible();
        await expect(page.locator('#bundleBtn')).toBeVisible();

        // Bundle button should be enabled after boot
        await expect(page.locator('#bundleBtn')).toBeEnabled();
    });

    test('bundles sample project with real Vite', async ({ page }) => {
        // Click to use sample project
        await page.click('#testBtn');

        // Check that input shows sample project
        await expect(page.locator('#urlInput')).toHaveValue('(Sample Vite Project)');

        // Click bundle button
        await page.click('#bundleBtn');

        // Wait for bundling to complete (this can take 2+ minutes for npm install)
        await expect(page.locator('#status')).toContainText('Bundle complete', { timeout: 300000 });

        // Check that output is not empty
        const outputArea = page.locator('#outputArea');
        const output = await outputArea.inputValue();

        expect(output.length).toBeGreaterThan(0);
        expect(output).toContain('<!DOCTYPE html>');

        // Verify it's a single file (no external references)
        expect(output).not.toContain('href="/src/style.css"');
        expect(output).not.toContain('src="/src/main.js"');

        // Check that CSS was inlined (should contain <style> with our styles)
        expect(output).toContain('<style>');
        expect(output).toContain('linear-gradient');

        // Check that JS was inlined
        expect(output).toContain('Count:');
    });

    test('stats are displayed after bundling', async ({ page }) => {
        // Use sample project
        await page.click('#testBtn');

        // Click bundle button
        await page.click('#bundleBtn');

        // Wait for bundling to complete
        await expect(page.locator('#status')).toContainText('Bundle complete', { timeout: 300000 });

        // Check stats are displayed
        const stats = page.locator('#stats');
        await expect(stats).toBeVisible();

        // Check individual stats have values
        await expect(page.locator('#bundleTime')).not.toHaveText('-');
        await expect(page.locator('#outputSize')).not.toHaveText('-');
    });

    test('copy button works', async ({ page }) => {
        // Use sample project
        await page.click('#testBtn');
        await page.click('#bundleBtn');

        // Wait for completion
        await expect(page.locator('#status')).toContainText('Bundle complete', { timeout: 300000 });

        // Copy button should be enabled
        await expect(page.locator('#copyBtn')).toBeEnabled();

        // Grant clipboard permissions
        await page.context().grantPermissions(['clipboard-write', 'clipboard-read']);

        // Click copy button
        await page.click('#copyBtn');

        // Check status message
        await expect(page.locator('#status')).toContainText('Copied to clipboard');
    });

    test('logs show Vite build progress', async ({ page }) => {
        // Use sample project
        await page.click('#testBtn');
        await page.click('#bundleBtn');

        // Check for npm install progress
        await expect(page.locator('#logOutput')).toContainText('npm install', { timeout: 60000 });

        // Wait for completion
        await expect(page.locator('#status')).toContainText('Bundle complete', { timeout: 300000 });

        // Check log entries for build steps
        const logOutput = page.locator('#logOutput');
        await expect(logOutput).toContainText('Dependencies installed');
        await expect(logOutput).toContainText('Vite build complete');
    });

});
