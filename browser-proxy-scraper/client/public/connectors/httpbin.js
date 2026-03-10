// httpbin connector — captures REST API responses from httpbin.org (no anti-bot)
await page.setData('status', 'Setting up network capture...');

// Register capture for httpbin.org API endpoints
await page.captureNetwork({
  urlPattern: 'httpbin\\.org',
  key: 'httpbin-api'
});

await page.setData('status', 'Navigating to httpbin.org...');
await page.goto('https://httpbin.org/');
await page.sleep(3000);

await page.setData('status', 'Fetching API data from httpbin.org...');

// Trigger several API calls from within the proxied page
await page.evaluate(`
  Promise.all([
    fetch('https://httpbin.org/get?test=hello').then(r => r.text()),
    fetch('https://httpbin.org/ip').then(r => r.text()),
    fetch('https://httpbin.org/user-agent').then(r => r.text()),
  ]).then(results => {
    console.log('Fetched httpbin data:', results.length, 'responses');
  }).catch(err => {
    console.error('Fetch error:', err);
  })
`);

await page.sleep(3000);

// Retrieve captured API responses
const apiData = await page.getCapturedResponse('httpbin-api');

if (apiData && apiData.length > 0) {
  await page.setData('result', {
    'httpbin.api': apiData,
    exportSummary: {
      count: apiData.length,
      label: 'API responses captured',
    },
    timestamp: new Date().toISOString(),
    platform: 'httpbin'
  });
  await page.setData('status', `Done! Captured ${apiData.length} API response(s)`);
} else {
  await page.setData('status', 'No API responses captured.');
}
