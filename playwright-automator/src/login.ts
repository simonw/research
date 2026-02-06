/**
 * Headed login capture: user completes login/2FA, then we persist storageState.
 */

import { chromium } from 'playwright';
import { mkdirSync } from 'node:fs';
import { createInterface } from 'node:readline';
import { writeAuthProfileMeta, getAuthProfilePaths } from './auth-profiles.js';

function waitForEnter(): Promise<void> {
  return new Promise((resolve) => {
    const rl = createInterface({ input: process.stdin, output: process.stdout });
    rl.question('Press ENTER when login/2FA is complete (and you are on the post-login page)...', () => {
      rl.close();
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
  console.log(`   After completing login/2FA, press ENTER in this terminal.\n`);

  await waitForEnter();

  await context.storageState({ path: storageStatePath });
  await context.close();

  writeAuthProfileMeta({
    domain: opts.domain,
    profileName: opts.profileName,
    createdAt: new Date().toISOString(),
    notes: opts.notes,
  }, opts.cwd);

  console.log(`\n✅ Saved storage state:`);
  console.log(`   ${storageStatePath}`);
}
