/**
 * Login — Open a headed browser for user to log in manually.
 *
 * Saves the resulting storage state for future replay.
 */

import { chromium } from '../browser/stealth.js';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { homedir } from 'node:os';

interface LoginOptions {
  chromePath?: string;
  headless?: boolean;
}

/**
 * Open a headed browser for the user to log in.
 * Returns the path to the saved storageState.json.
 */
export async function login(url: string, options: LoginOptions = {}): Promise<string> {
  const { chromePath, headless = false } = options;

  let domain: string;
  try {
    domain = new URL(url).hostname;
  } catch {
    throw new Error(`Invalid URL: ${url}`);
  }

  // Storage location
  const authDir = join(homedir(), '.data-agent', 'auth', domain.replace(/[^a-zA-Z0-9.-]/g, '_'));
  if (!existsSync(authDir)) mkdirSync(authDir, { recursive: true });
  const storageStatePath = join(authDir, 'storageState.json');

  console.log(`Opening browser for login at ${url}...`);
  console.log('Please log in, complete any 2FA, and close the browser when done.\n');

  const launchOptions: Record<string, unknown> = { headless };
  if (chromePath) launchOptions.executablePath = chromePath;

  const browser = await chromium.launch(launchOptions);
  const context = await browser.newContext();
  const page = await context.newPage();

  await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30_000 });

  // Wait for the user to close the browser
  await new Promise<void>((resolve) => {
    browser.on('disconnected', () => resolve());

    // Also check periodically if the page was closed
    const interval = setInterval(async () => {
      try {
        const pages = context.pages();
        if (pages.length === 0) {
          clearInterval(interval);
          resolve();
        }
      } catch {
        clearInterval(interval);
        resolve();
      }
    }, 1000);
  });

  // Save storage state before close
  try {
    const state = await context.storageState();
    writeFileSync(storageStatePath, JSON.stringify(state, null, 2));
    console.log(`\nAuth saved to ${storageStatePath}`);
  } catch {
    console.log('\nCould not save storage state (browser already closed)');
  }

  try { await browser.close(); } catch { /* already closed */ }

  // Save metadata
  const metaPath = join(authDir, 'meta.json');
  writeFileSync(metaPath, JSON.stringify({
    profileName: 'default',
    domain,
    createdAt: new Date().toISOString(),
  }, null, 2));

  return storageStatePath;
}
