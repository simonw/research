// @ts-check
const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
    testDir: './',
    testMatch: '*.test.js',
    timeout: 300000, // 5 minutes per test
    retries: 0,
    workers: 1, // Run serially for consistent benchmark results
    reporter: [
        ['list'],
        ['html', { outputFolder: 'test-results' }],
    ],
    use: {
        headless: true,
        viewport: { width: 1280, height: 720 },
        ignoreHTTPSErrors: true,
    },
    projects: [
        {
            name: 'chromium',
            use: {
                browserName: 'chromium',
            },
        },
    ],
});
