import { chromium } from 'playwright-extra';
import StealthPlugin from 'puppeteer-extra-plugin-stealth';
import type { BrowserContext, Page } from 'playwright';
import { ensureAutomationProfile } from './profile.js';

chromium.use(StealthPlugin());

export { chromium };

export interface LaunchProfileOptions {
  headless?: boolean;
  chromePath?: string;
  recordHar?: { path: string; mode?: 'full' | 'minimal' };
  skipSync?: boolean;
}

export interface ProfileBrowser {
  context: BrowserContext;
  page: Page;
  close: () => Promise<void>;
}

/**
 * Launch a persistent browser context backed by the automation profile.
 *
 * Syncs auth-critical files from the user's Chrome profile, then opens
 * a persistent context so cookies/localStorage/IndexedDB are available.
 */
export async function launchProfile(options?: LaunchProfileOptions): Promise<ProfileBrowser> {
  const { headless = false, chromePath, recordHar, skipSync } = options ?? {};

  const profileDir = await ensureAutomationProfile({ skipSync });

  const contextOptions: Record<string, unknown> = { headless };
  if (chromePath) contextOptions.executablePath = chromePath;
  if (recordHar) contextOptions.recordHar = recordHar;

  const context = await chromium.launchPersistentContext(profileDir, contextOptions);

  // Persistent contexts pre-create a page — reuse it
  const page = context.pages()[0] ?? await context.newPage();

  return {
    context,
    page,
    close: () => context.close(),
  };
}
