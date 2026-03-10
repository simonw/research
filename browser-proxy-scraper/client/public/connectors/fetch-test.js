// Fetch test connector — self-contained fetch test using jsonplaceholder (no anti-bot)
await page.setData('status', 'Setting up network capture...');

// Register capture for jsonplaceholder responses
await page.captureNetwork({
  urlPattern: 'jsonplaceholder',
  key: 'jsonplaceholder'
});

await page.setData('status', 'Navigating to example.com...');
await page.goto('https://example.com/');
await page.sleep(2000);

await page.setData('status', 'Triggering fetch requests from within the page...');

// Fire multiple fetch requests to jsonplaceholder from within the proxied page
await page.evaluate(`
  Promise.all([
    fetch('https://jsonplaceholder.typicode.com/posts/1').then(r => r.json()),
    fetch('https://jsonplaceholder.typicode.com/users/1').then(r => r.json()),
  ]).then(results => {
    console.log('Fetch results:', JSON.stringify(results));
  }).catch(err => {
    console.error('Fetch error:', err);
  })
`);

await page.sleep(3000);

// Retrieve captured responses
const fetchData = await page.getCapturedResponse('jsonplaceholder');

if (fetchData && fetchData.length > 0) {
  await page.setData('result', {
    'jsonplaceholder.responses': fetchData,
    exportSummary: {
      count: fetchData.length,
      label: 'Fetch responses captured',
    },
    timestamp: new Date().toISOString(),
    platform: 'jsonplaceholder'
  });
  await page.setData('status', `Done! Captured ${fetchData.length} fetch response(s)`);
} else {
  await page.setData('status', 'No fetch responses captured. The service worker may not have intercepted cross-origin requests.');
}
