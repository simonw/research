/**
 * Detect system Chrome/Chromium/Edge executable path.
 *
 * We use the system browser to avoid downloading Chromium (~400MB).
 * Falls back to Playwright's bundled browser if no system browser found.
 */

import { existsSync } from 'node:fs';
import { execSync } from 'node:child_process';
import { platform } from 'node:os';

/** Well-known Chrome/Chromium/Edge paths by platform. */
const BROWSER_PATHS: Record<string, string[]> = {
  darwin: [
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
  ],
  linux: [
    '/usr/bin/google-chrome',
    '/usr/bin/google-chrome-stable',
    '/usr/bin/chromium',
    '/usr/bin/chromium-browser',
    '/snap/bin/chromium',
    '/usr/bin/microsoft-edge',
  ],
  win32: [
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
    `${process.env.LOCALAPPDATA ?? ''}\\Google\\Chrome\\Application\\chrome.exe`,
    'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
  ],
};

/**
 * Detect system Chrome/Chromium/Edge executable.
 * Returns the path if found, undefined otherwise.
 */
export function detectSystemChrome(): string | undefined {
  const os = platform();
  const candidates = BROWSER_PATHS[os] ?? [];

  for (const path of candidates) {
    if (existsSync(path)) {
      return path;
    }
  }

  // Try `which` on Unix-like systems
  if (os !== 'win32') {
    for (const cmd of ['google-chrome', 'chromium', 'chromium-browser', 'microsoft-edge']) {
      try {
        const result = execSync(`which ${cmd}`, { encoding: 'utf-8' }).trim();
        if (result && existsSync(result)) {
          return result;
        }
      } catch {
        // Not found
      }
    }
  }

  return undefined;
}

/**
 * Get Chrome executable path from env, system detection, or undefined.
 */
export function getChromePath(): string | undefined {
  // 1. Environment variable override
  if (process.env.CHROME_PATH && existsSync(process.env.CHROME_PATH)) {
    return process.env.CHROME_PATH;
  }

  // 2. System detection
  return detectSystemChrome();
}
