const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
  testDir: '.',
  testMatch: 'tests/*.spec.js',
  timeout: 180000,
  use: {
    headless: true,
  },
  projects: [
    {
      name: 'firefox',
      use: {
        browserName: 'firefox',
      },
    },
  ],
  webServer: {
    command: 'node serve.js',
    port: 8765,
    cwd: __dirname,
    reuseExistingServer: true,
    timeout: 10000,
  },
});
