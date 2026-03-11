// Google login connector — checks if Google login form renders
await page.setData('status', 'Navigating to Google login...');
await page.goto('https://accounts.google.com/');
await page.sleep(5000);

await page.setData('status', 'Checking if login form rendered...');

const result = await page.evaluate(`
  JSON.stringify({
    title: document.title,
    url: window.location.href,
    // Check for email/identifier input
    hasIdentifierInput: !!document.querySelector('input[type="email"]') ||
      !!document.querySelector('#identifierId'),
    // Check for "Next" button
    hasNextButton: !!document.querySelector('#identifierNext'),
    // Check for Google branding
    hasGoogleBranding: !!document.querySelector('#googleLogo') ||
      document.title.includes('Google'),
    // Check for error/block page
    isBlocked: document.body ? (
      document.body.innerText.includes('unusual traffic') ||
      document.body.innerText.includes('browser isn\\'t supported')
    ) : false,
    bodyText: document.body ? document.body.innerText.substring(0, 500) : '',
  })
`);

const parsed = JSON.parse(result);
await page.setData('result', {
  google: parsed,
  timestamp: new Date().toISOString(),
  platform: 'google'
});

if (parsed.hasIdentifierInput) {
  await page.setData('status', 'Google login form rendered successfully!');
} else if (parsed.isBlocked) {
  await page.setData('status', 'Google detected unusual traffic or unsupported browser.');
} else {
  await page.setData('status', 'Login form not found. Page may have redirected.');
}
