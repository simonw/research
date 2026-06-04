// @ts-check
const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
    testDir: './tests',
    fullyParallel: false,
    forbidOnly: !!process.env.CI,
    retries: 0,
    workers: 1,
    reporter: 'list',
    timeout: 300000, // 5 minutes for WebContainer tests
    use: {
        baseURL: 'http://localhost:3000',
        trace: 'off',
        video: 'off',
        screenshot: 'off',
        headless: true,
    },
    projects: [
        {
            name: 'chromium',
            testMatch: /bundler-simple\.spec\.js/,
            use: {
                browserName: 'chromium',
                launchOptions: {
                    args: [
                        '--no-sandbox',
                        '--disable-setuid-sandbox',
                        '--disable-dev-shm-usage',
                        '--disable-gpu',
                        '--disable-software-rasterizer',
                        '--disable-extensions',
                        '--single-process',
                        '--no-zygote',
                    ],
                    headless: true,
                },
            },
        },
        {
            name: 'chromium-webcontainer',
            testMatch: /bundler-webcontainer\.spec\.js/,
            use: {
                browserName: 'chromium',
                launchOptions: {
                    args: [
                        '--no-sandbox',
                        '--disable-setuid-sandbox',
                        '--disable-dev-shm-usage',
                        '--disable-gpu',
                        '--disable-software-rasterizer',
                        '--disable-extensions',
                        '--single-process',
                        '--no-zygote',
                    ],
                    headless: true,
                },
            },
        },
    ],
    webServer: {
        command: 'node server-coi.js',
        url: 'http://localhost:3000',
        reuseExistingServer: true,
        timeout: 30 * 1000,
    },
});
