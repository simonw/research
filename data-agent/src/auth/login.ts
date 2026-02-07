/**
 * Login — Open a persistent-profile browser for user to log in manually.
 *
 * Auth persists in ~/.data-agent/browser-profile/ automatically.
 */

import { launchProfile } from '../browser/stealth.js';

interface LoginOptions {
  chromePath?: string;
  headless?: boolean;
}

/**
 * Open a persistent-profile browser for the user to log in.
 * Cookies and storage persist in the automation profile directory.
 */
export async function login(url: string, options: LoginOptions = {}): Promise<void> {
  const { chromePath, headless = false } = options;

  try {
    new URL(url);
  } catch {
    throw new Error(`Invalid URL: ${url}`);
  }

  console.log(`Opening browser for login at ${url}...`);
  console.log('Please log in, complete any 2FA, and close the browser when done.\n');

  const { context, page, close } = await launchProfile({ chromePath, headless });

  await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30_000 });

  // Wait for the user to close all pages (persistent contexts stay alive)
  await new Promise<void>((resolve) => {
    context.on('close', () => resolve());

    // Poll for all pages closed — persistent contexts don't auto-close
    const interval = setInterval(() => {
      try {
        if (context.pages().length === 0) {
          clearInterval(interval);
          close().then(() => resolve(), () => resolve());
        }
      } catch {
        clearInterval(interval);
        resolve();
      }
    }, 1000);
  });

  console.log('\nAuth saved to persistent browser profile.');
}
