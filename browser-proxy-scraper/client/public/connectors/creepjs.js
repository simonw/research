// CreepJS connector — captures browser fingerprint analysis results
await page.setData('status', 'Navigating to CreepJS...');
await page.goto('https://abrahamjuliot.github.io/creepjs/');
await page.sleep(10000);

await page.setData('status', 'Waiting for fingerprint analysis to complete...');
await page.sleep(5000);

const result = await page.evaluate(`
  JSON.stringify({
    title: document.title,
    url: window.location.href,
    // Try to capture the fingerprint trust score
    trustScore: (document.querySelector('.visitor-info .grade') ||
      document.querySelector('[class*="grade"]') || {}).textContent || null,
    // Capture the fingerprint ID if available
    fingerprintId: (document.querySelector('.visitor-info .fingerprint') ||
      document.querySelector('[class*="fingerprint"]') || {}).textContent || null,
    // Check if analysis completed (look for results sections)
    hasResults: !!document.querySelector('.fingerprint-header') ||
      !!document.querySelector('[class*="visitor"]') ||
      document.querySelectorAll('table').length > 0,
    tableCount: document.querySelectorAll('table').length,
    // Capture key summary text
    summaryText: document.body ? document.body.innerText.substring(0, 1000) : '',
  })
`);

const parsed = JSON.parse(result);
await page.setData('result', {
  creepjs: parsed,
  timestamp: new Date().toISOString(),
  platform: 'creepjs'
});

if (parsed.hasResults) {
  const score = parsed.trustScore ? ` (Trust: ${parsed.trustScore})` : '';
  await page.setData('status', `CreepJS analysis complete!${score}`);
} else {
  await page.setData('status', 'CreepJS page loaded but analysis results not found. May need more time.');
}
