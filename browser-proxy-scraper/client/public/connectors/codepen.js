// CodePen connector — captures GraphQL API responses
await page.setData('status', 'Setting up network capture...');

// Register capture for CodePen's GraphQL endpoint
await page.captureNetwork({
  urlPattern: '/graphql',
  key: 'graphql'
});

await page.setData('status', 'Navigating to CodePen...');
await page.goto('https://codepen.io/');
await page.sleep(5000);

// Check what loaded
const info = await page.evaluate(`
  JSON.stringify({
    title: document.title,
    url: window.location.href,
    bodyLength: document.body ? document.body.innerHTML.length : 0
  })
`);
await page.setData('status', 'Page loaded: ' + info);

// Wait a bit more for GraphQL requests to complete
await page.sleep(3000);

// Retrieve captured GraphQL responses
const graphqlData = await page.getCapturedResponse('graphql');

if (graphqlData && graphqlData.length > 0) {
  await page.setData('result', {
    'codepen.graphql': graphqlData,
    exportSummary: {
      count: graphqlData.length,
      label: 'GraphQL responses captured',
    },
    timestamp: new Date().toISOString(),
    platform: 'codepen'
  });
  await page.setData('status', `Done! Captured ${graphqlData.length} GraphQL response(s)`);
} else {
  await page.setData('status', 'No GraphQL responses captured. Try browsing the page manually.');
}
