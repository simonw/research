/**
 * Headed login capture: user completes login/2FA, then we persist storageState.
 */

import { chromium } from 'playwright';
import { mkdirSync } from 'node:fs';
import { writeAuthProfileMeta, getAuthProfilePaths } from './auth-profiles.js';

/** Wait for the user to press Enter in the terminal (silent — no prompt printed). */
function waitForEnter(): Promise<void> {
  return new Promise((resolve) => {
    const { stdin } = process;
    const wasRaw = stdin.isRaw;

    if (stdin.isTTY) {
      stdin.setRawMode(false);
    }

    stdin.resume();
    stdin.once('data', () => {
      if (stdin.isTTY && wasRaw !== undefined) {
        try { stdin.setRawMode(wasRaw); } catch { /* ignore */ }
      }
      stdin.pause();
      resolve();
    });
  });
}

export async function captureLoginStorageState(opts: {
  url: string;
  domain: string;
  profileName: string;
  headless?: boolean;
  notes?: string;
  cwd?: string;
}) {
  const { dir, storageStatePath } = getAuthProfilePaths(opts.domain, opts.profileName, opts.cwd);
  mkdirSync(dir, { recursive: true });

  const context = await chromium.launchPersistentContext(dir + '/user-data', {
    headless: opts.headless ?? false,
    args: [
      '--disable-blink-features=AutomationControlled',
      '--no-sandbox',
      '--disable-dev-shm-usage',
    ],
    viewport: { width: 1280, height: 800 },
  });

  const page = await context.newPage();
  await page.goto(opts.url, { waitUntil: 'domcontentloaded', timeout: 60000 }).catch(() => {});

  console.log(`\n🔐 Login capture started for domain: ${opts.domain}`);
  console.log(`   Profile: ${opts.profileName}`);
  console.log(`   After completing login/2FA, close the browser or press ENTER.\n`);

  // Wait for user to press Enter OR close the browser
  await Promise.race([
    waitForEnter(),
    new Promise<void>((resolve) => {
      page.on('close', () => resolve());
    }),
  ]);

  // These may fail if the user closed the browser — fall back gracefully.
  try { await context.storageState({ path: storageStatePath }); } catch { /* browser already closed */ }
  try { await context.close(); } catch { /* already closed */ }

  writeAuthProfileMeta({
    domain: opts.domain,
    profileName: opts.profileName,
    createdAt: new Date().toISOString(),
    notes: opts.notes,
  }, opts.cwd);

  console.log(`\n✅ Saved storage state:`);
  console.log(`   ${storageStatePath}`);
}
