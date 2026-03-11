// Instagram login connector — tests Cloudflare bypass by checking if login form renders
await page.setData('status', 'Navigating to Instagram login...');
await page.goto('https://www.instagram.com/accounts/login/');
await page.sleep(5000);

await page.setData('status', 'Checking if login form rendered...');

const result = await page.evaluate(`
  JSON.stringify({
    title: document.title,
    url: window.location.href,
    // Check for login form elements
    hasUsernameInput: !!document.querySelector('input[name="username"]'),
    hasPasswordInput: !!document.querySelector('input[name="password"]'),
    hasLoginButton: !!document.querySelector('button[type="submit"]'),
    // Check for Cloudflare challenge page
    isCloudflareChallenge: document.body ? (
      document.body.innerText.includes('Checking your browser') ||
      document.body.innerText.includes('Just a moment')
    ) : false,
    bodyText: document.body ? document.body.innerText.substring(0, 500) : '',
  })
`);

const parsed = JSON.parse(result);
await page.setData('result', {
  instagram: parsed,
  timestamp: new Date().toISOString(),
  platform: 'instagram'
});

if (parsed.hasUsernameInput && parsed.hasPasswordInput) {
  await page.setData('status', 'Instagram login form rendered — Cloudflare bypass successful!');
} else if (parsed.isCloudflareChallenge) {
  await page.setData('status', 'Stuck on Cloudflare challenge page.');
} else {
  await page.setData('status', 'Login form not found. Page may have redirected or blocked.');
}
