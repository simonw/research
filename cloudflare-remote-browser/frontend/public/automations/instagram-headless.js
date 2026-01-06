const state = {
  webInfo: null,
  profileData: null,
  timelineEdges: [],
  pageInfo: null,
  totalFetched: 0,
  isProfileComplete: false,
  isTimelineComplete: false,
  isComplete: false
};

const fetchWebInfo = async () => {
  try {
    const result = await page.evaluate(`
      (async () => {
        try {
          const response = await fetch("https://www.instagram.com/accounts/web_info/", {
            headers: { "X-Requested-With": "XMLHttpRequest" }
          });
          if (!response.ok) return { error: 'response not ok', status: response.status };

          const html = await response.text();
          const parser = new DOMParser();
          const doc = parser.parseFromString(html, "text/html");
          const scripts = doc.querySelectorAll('script[type="application/json"][data-sjs]');

          const findPolarisData = (obj) => {
            if (!obj || typeof obj !== 'object') return null;
            if (Array.isArray(obj) && obj[0] === 'PolarisViewer' && obj.length >= 3) {
              return obj[2];
            }
            for (const key in obj) {
              if (Object.prototype.hasOwnProperty.call(obj, key)) {
                const found = findPolarisData(obj[key]);
                if (found) return found;
              }
            }
            return null;
          };

          let foundData = null;
          for (const script of scripts) {
            try {
              const jsonContent = JSON.parse(script.textContent);
              foundData = findPolarisData(jsonContent);
              if (foundData) break;
            } catch (e) {}
          }

          if (foundData && foundData.data) {
            return { success: true, data: foundData.data };
          }
          return { error: 'no polaris data found', scriptsCount: scripts.length };
        } catch (err) {
          return { error: err.message };
        }
      })()
    `);
    if (result?.success) {
      return result.data;
    }
    return null;
  } catch (err) {
    return null;
  }
};

await page.setData('status', 'Launching Instagram...');

let isLoggedIn = false;
let lastError = null;
let attempts = 0;
const maxAttempts = 3;
let credentials = null;
await page.goto('https://www.instagram.com/');

const clickNext = async () => {
  try {
    const nextBtn = 'button:has-text("Next"), [role="button"]:has-text("Next"), button:has-text("Continue"), [role="button"]:has-text("Continue"), button:has-text("Send Security Code")';

    try {
      await page.waitForFunction(`
          (() => {
            const btn = Array.from(document.querySelectorAll('button, [role="button"]'))
              .find(el => /Next|Continue|Send Security Code/i.test(el.innerText || ''));
            return btn && btn.getAttribute('aria-disabled') !== 'true';
          })()
        `, { timeout: 5000 });
    } catch (e) { }

    await page.click(nextBtn, { timeout: 5000 });
    return true;
  } catch (e) {
    return false;
  }
};

const detectChallenge = async () => {
  try {
    await page.setData('status', 'Checking for Captcha...');

    const info = await page.evaluate(`
        (() => {
          return {
            text: document.body.innerText,
            hasCaptchaSelectors: !!document.querySelector('.g-recaptcha, [data-sitekey], iframe[src*="recaptcha"], iframe[src*="captcha"]'),
            url: window.location.href
          };
        })()
      `);
    return info?.text?.includes("Help us confirm it's you") || info?.hasCaptchaSelectors || false;
  } catch (e) {
    await page.setData('status', 'Detection error: ' + e.message);
    return false;
  }
};

const detectOtp = async () => {
  try {
    return await page.evaluate(`
        (() => {
          const text = document.body.innerText;
          const hasCodeInput = !!document.querySelector('input[name="verificationCode"], input[name="email"][id^="_r_"], input[aria-label*="Code"]');
          return hasCodeInput && (
            text.includes("Enter the code") ||
            text.includes("Check your email") ||
            text.includes("Two-factor authentication") ||
            text.includes("Security code")
          );
        })()
      `);
  } catch (e) {
    return false;
  }
};

while (!isLoggedIn && attempts < maxAttempts) {
  attempts++;

  // Wait for login page to load
  try {
    const userSelector = 'input[name="username"], input[name="email"], input[aria-label*="Username"]';
    await page.waitForSelector(userSelector, { timeout: 10000, state: 'visible' });
  } catch (e) {
    lastError = "Failed to load Instagram login page. Retrying...";
    continue;
  }

  // Get credentials from user
  if (!credentials || lastError) {
    credentials = await page.getInput({
      title: 'Log in to Instagram',
      description: lastError ? lastError : 'Enter your Instagram credentials to continue',
      schema: {
        type: 'object',
        required: ['username', 'password'],
        properties: {
          username: { type: 'string', title: 'Username, Email or Phone' },
          password: { type: 'string', title: 'Password' }
        }
      },
      uiSchema: {
        username: { 'ui:placeholder': 'Username, email, or phone', 'ui:autofocus': true },
        password: { 'ui:widget': 'password', 'ui:placeholder': 'Password' }
      },
      submitLabel: 'Log In',
      error: lastError
    });
  }

  await page.setData('status', 'Signing in...');

  try {
    const userSelector = 'input[name="username"], input[name="email"], input[aria-label*="Username"]';
    const passSelector = 'input[name="password"], input[name="pass"], input[aria-label*="Password"]';
    await page.fill(userSelector, credentials.username);
    await page.fill(passSelector, credentials.password);
    await page.click('button[type="submit"]');
  } catch (e) {
    lastError = "Login form disappeared or became unresponsive. Retrying...";
    continue;
  }

  await page.setData('status', 'Authenticating...');
  await page.sleep(8000);

  if (await detectChallenge()) {
    await page.setData('status', 'Captcha detected...');

    // If there's a "Next" button on the captcha explainer, click it first
    /* try {
      const hasNext = await page.evaluate(`
        !!Array.from(document.querySelectorAll('button, [role="button"]'))
          .find(el => /Next|Continue/i.test(el.innerText || ''))
      `);
      if (hasNext) {
        await page.setData('status', 'Navigating through security check...');
        await clickNext();
        await page.sleep(3000);
      }
    } catch (e) { } */

    await page.setData('status', 'Solving captcha...');
    try {
      console.log('[instagram] Calling solveCaptcha with enterprise=true');
      const captchaResult = await page.solveCaptcha({ enterprise: true });
      console.log('[instagram] solveCaptcha result:', JSON.stringify(captchaResult));

      if (captchaResult.skipped) {
        console.log('[instagram] Captcha was skipped (not detected)');
        await page.setData('status', 'No captcha detected, continuing...');
      } else if (captchaResult.fallback) {
        console.log('[instagram] Falling back to manual solve');
        await page.setData('status', 'Please complete the security check in the browser view.');
      } else if (captchaResult.success) {
        console.log('[instagram] Captcha solved successfully');
        await page.setData('status', 'Challenge solved, waiting for redirect...');

        // Wait a bit for the captcha to be processed by the site's script
        await page.sleep(3000);

        // Try clicking next
        await clickNext();

        // Verify if we're still on the challenge page
        /* const stillOnChallenge = await page.evaluate(`
          document.body.innerText.includes("Help us confirm it's you") ||
          document.body.innerText.includes("reCAPTCHA") ||
          !!document.querySelector('iframe[src*="recaptcha"]')
        `); */

        if (await detectChallenge()) {
          /* console.log('[instagram] Still on challenge page, trying to click any button...');
          await page.evaluate(`
            (() => {
              const buttons = Array.from(document.querySelectorAll('button, [role="button"]'));
              const nextBtn = buttons.find(b => /Next|Continue|Confirm|Verify/i.test(b.innerText || ''));
              if (nextBtn) nextBtn.click();
            })()
          `);
          await page.sleep(5000); */
          clickNext();
        }
      } else {
        console.log('[instagram] Unexpected captcha result:', captchaResult);
        await page.setData('status', 'Captcha result: ' + JSON.stringify(captchaResult));
      }
    } catch (e) {
      console.error('[instagram] solveCaptcha error:', e);
      await page.setData('status', `Captcha error: ${e.message || String(e)}`);
      await page.setData('captcha_error', e.message || String(e));
    }
    await page.sleep(10000);
  }

  // Check for login error (eg: invalid credentials)
  let errorMsg = null;
  try {
    errorMsg = await page.evaluate(`
        (() => {
          const el = document.querySelector('[role="alert"], #loginForm p[role="alert"]');
          return el ? el.textContent : null;
        })()
      `);
  } catch (e) { }

  if (errorMsg) {
    lastError = `Login failed: ${errorMsg}`;
    continue;
  }

  if (await detectOtp()) {
    await page.setData('status', 'Two-factor authentication required');
    const otpResult = await page.getInput({
      title: 'Two-Factor Authentication',
      description: 'Enter the security code sent to your email or phone',
      schema: {
        type: 'object',
        required: ['code'],
        properties: {
          code: { type: 'string', title: 'Security Code', minLength: 6, maxLength: 8 }
        }
      },
      uiSchema: { code: { 'ui:placeholder': '000000', 'ui:autofocus': true } },
      submitLabel: 'Verify'
    });

    try {
      const otpSelector = 'input[name="verificationCode"], input[name="email"], input[aria-label*="Code"]';
      await page.fill(otpSelector, otpResult.code);

      const submitBtn = 'button:has-text("Confirm"), button:has-text("Verify"), [role="button"]:has-text("Continue")';
      await page.click(submitBtn);
      await page.sleep(8000);
    } catch (e) { }
  }

  const webInfo = await fetchWebInfo();
  if (webInfo?.username) {
    isLoggedIn = true;
    state.webInfo = webInfo;
    await page.setData('status', `Logged in as @${webInfo.username}`);
  } else {
    const url = await page.url();
    if (url.includes('challenge') || url.includes('checkpoint')) {
      await page.setData('status', 'Final security check...');
      try {
        console.log('[instagram] Final security check - calling solveCaptcha');
        const finalCaptchaResult = await page.solveCaptcha({ enterprise: true });
        console.log('[instagram] Final solveCaptcha result:', JSON.stringify(finalCaptchaResult));

        if (finalCaptchaResult.fallback) {
          await page.setData('status', 'Please complete the final security check manually.');
        } else if (finalCaptchaResult.success) {
          await page.setData('status', 'Final security check solved');
        }
      } catch (e) {
        console.error('[instagram] Final solveCaptcha error:', e);
        await page.setData('status', `Final captcha error: ${e.message || String(e)}`);
      }
      await page.sleep(8000);
      const finalInfo = await fetchWebInfo();
      if (finalInfo?.username) {
        isLoggedIn = true;
        state.webInfo = finalInfo;
      }
    } else {
      await page.goto('https://www.instagram.com/');
      await page.sleep(5000);
      const retryInfo = await fetchWebInfo();
      if (retryInfo?.username) {
        isLoggedIn = true;
        state.webInfo = retryInfo;
      }
    }

    if (!isLoggedIn) {
      lastError = 'Login failed. Please check credentials or security status.';
    }
  }
}

if (!isLoggedIn) {
  await page.setData('error', 'Login process failed');
  return { success: false, error: 'Login process failed' };
}

const username = state.webInfo?.username;
await page.setData('status', `Fetching profile: @${username}`);

await page.captureNetwork({
  urlPattern: '/graphql',
  bodyPattern: 'PolarisProfilePageContentQuery|ProfilePageQuery|UserByUsernameQuery',
  key: 'profileResponse'
});

await page.goto(`https://www.instagram.com/${username}/`);
await page.sleep(5000);

let profileData = await page.getCapturedResponse('profileResponse');
if (profileData) {
  const userData = profileData?.data?.data?.user;
  if (userData) {
    state.profileData = {
      username: userData.username,
      full_name: userData.full_name,
      bio: userData.biography,
      followers: userData.follower_count,
      following: userData.following_count,
      media_count: userData.media_count,
      webInfo: state.webInfo,
      platform: 'instagram',
      timestamp: new Date().toISOString()
    };
    await page.setData('result', state.profileData);
    return { success: true, data: state.profileData };
  }
}

return { success: false, error: 'Failed to capture data' };
