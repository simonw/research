// hCaptcha demo connector — checks for auto-verification success
await page.setData('status', 'Navigating to hCaptcha demo...');
await page.goto('https://accounts.hcaptcha.com/demo');
await page.sleep(5000);

await page.setData('status', 'Checking hCaptcha widget status...');

const result = await page.evaluate(`
  JSON.stringify({
    title: document.title,
    url: window.location.href,
    // Check for hCaptcha iframe
    hasHcaptchaFrame: !!document.querySelector('iframe[src*="hcaptcha"]'),
    // Check for hCaptcha widget container
    hasHcaptchaWidget: !!document.querySelector('.h-captcha'),
    // Check for success message on page
    hasSuccessMessage: document.body ? document.body.innerText.includes('success') : false,
    // Check for response token
    hasResponseToken: !!(document.querySelector('[name="h-captcha-response"]') &&
      document.querySelector('[name="h-captcha-response"]').value),
    bodyText: document.body ? document.body.innerText.substring(0, 500) : '',
  })
`);

const parsed = JSON.parse(result);
await page.setData('result', {
  hcaptcha: parsed,
  timestamp: new Date().toISOString(),
  platform: 'hcaptcha-demo'
});

if (parsed.hasSuccessMessage || parsed.hasResponseToken) {
  await page.setData('status', 'hCaptcha auto-verification succeeded!');
} else if (parsed.hasHcaptchaWidget || parsed.hasHcaptchaFrame) {
  await page.setData('status', 'hCaptcha widget rendered but auto-verification did not complete.');
} else {
  await page.setData('status', 'hCaptcha widget not found on page.');
}
