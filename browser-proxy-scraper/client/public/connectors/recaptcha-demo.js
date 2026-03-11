// reCAPTCHA demo connector — checks if reCAPTCHA widget renders without domain errors
await page.setData('status', 'Navigating to reCAPTCHA demo...');
await page.goto('https://www.google.com/recaptcha/api2/demo');
await page.sleep(5000);

await page.setData('status', 'Checking reCAPTCHA widget...');

const result = await page.evaluate(`
  JSON.stringify({
    title: document.title,
    url: window.location.href,
    // Check for reCAPTCHA iframe
    hasRecaptchaFrame: !!document.querySelector('iframe[src*="recaptcha"]'),
    // Check for reCAPTCHA widget container
    hasRecaptchaWidget: !!document.querySelector('.g-recaptcha'),
    // Check for domain error message
    hasDomainError: document.body ? document.body.innerText.includes('ERROR for site owner') : false,
    // Check for response token
    hasResponseToken: !!(document.querySelector('#g-recaptcha-response') &&
      document.querySelector('#g-recaptcha-response').value),
    bodyText: document.body ? document.body.innerText.substring(0, 500) : '',
  })
`);

const parsed = JSON.parse(result);
await page.setData('result', {
  recaptcha: parsed,
  timestamp: new Date().toISOString(),
  platform: 'recaptcha-demo'
});

if (parsed.hasDomainError) {
  await page.setData('status', 'reCAPTCHA rendered with domain error (expected through proxy).');
} else if (parsed.hasRecaptchaFrame || parsed.hasRecaptchaWidget) {
  await page.setData('status', 'reCAPTCHA widget rendered successfully!');
} else {
  await page.setData('status', 'reCAPTCHA widget not found on page.');
}
