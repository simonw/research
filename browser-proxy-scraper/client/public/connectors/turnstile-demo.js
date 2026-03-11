// Turnstile demo connector — tests Cloudflare Turnstile CAPTCHA widget
await page.setData('status', 'Navigating to Turnstile demo...');
await page.goto('https://turnstiledemo.lusostreams.com/');
await page.sleep(5000);

await page.setData('status', 'Checking Turnstile widget status...');

const result = await page.evaluate(`
  JSON.stringify({
    title: document.title,
    url: window.location.href,
    // Check for Turnstile iframe
    hasTurnstileFrame: !!document.querySelector('iframe[src*="turnstile"]'),
    // Check for success/verification element
    hasSuccessToken: !!(document.querySelector('[name="cf-turnstile-response"]') &&
      document.querySelector('[name="cf-turnstile-response"]').value),
    // Check for visible widget
    hasTurnstileWidget: !!document.querySelector('.cf-turnstile'),
    bodyText: document.body ? document.body.innerText.substring(0, 500) : '',
  })
`);

const parsed = JSON.parse(result);
await page.setData('result', {
  turnstile: parsed,
  timestamp: new Date().toISOString(),
  platform: 'turnstile-demo'
});

if (parsed.hasSuccessToken) {
  await page.setData('status', 'Turnstile verification completed successfully!');
} else if (parsed.hasTurnstileWidget || parsed.hasTurnstileFrame) {
  await page.setData('status', 'Turnstile widget rendered but verification did not auto-complete.');
} else {
  await page.setData('status', 'Turnstile widget not found on page.');
}
