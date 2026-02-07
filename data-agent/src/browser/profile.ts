/**
 * Chrome profile sync — copies auth-critical files from a real Chrome profile
 * into an automation-safe profile directory at ~/.data-agent/browser-profile/.
 *
 * This gives Playwright access to real cookies, localStorage, and IndexedDB
 * without touching the user's actual Chrome profile.
 */

import { existsSync, mkdirSync, cpSync, copyFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { homedir, platform } from 'node:os';

/**
 * Locate the Chrome user-data directory on this OS.
 * Returns undefined if Chrome is not installed.
 */
export function getChromeUserDataDir(): string | undefined {
  const os = platform();
  let dir: string;

  if (os === 'darwin') {
    dir = join(homedir(), 'Library', 'Application Support', 'Google', 'Chrome');
  } else if (os === 'linux') {
    dir = join(homedir(), '.config', 'google-chrome');
  } else if (os === 'win32') {
    dir = join(homedir(), 'AppData', 'Local', 'Google', 'Chrome', 'User Data');
  } else {
    return undefined;
  }

  return existsSync(dir) ? dir : undefined;
}

/**
 * Returns the path to the automation profile directory (~/.data-agent/browser-profile/).
 */
export function getAutomationProfileDir(): string {
  return join(homedir(), '.data-agent', 'browser-profile');
}

/** Files/dirs to skip when syncing (caches, GPU data, service workers). */
const SKIP_PATTERNS = [
  'Cache',
  'Code Cache',
  'GPUCache',
  'Service Worker',
  'blob_storage',
  'GrShaderCache',
  'ShaderCache',
  'component_crx_cache',
];

/**
 * Sync auth-critical files from a Chrome profile to the automation profile.
 *
 * Copies: Cookies, Cookies-journal, Local Storage/, IndexedDB/, Preferences, Local State.
 * Skips caches and large non-auth directories.
 */
export async function syncProfile(opts?: {
  chromeProfile?: string;
  skipSync?: boolean;
}): Promise<{ synced: boolean }> {
  if (opts?.skipSync) return { synced: false };

  const chromeDir = getChromeUserDataDir();
  if (!chromeDir) {
    console.log('  Profile sync: Chrome not found, skipping sync');
    return { synced: false };
  }

  let profileName = opts?.chromeProfile ?? process.env.CHROME_PROFILE ?? 'Default';
  let srcProfile = join(chromeDir, profileName);

  // Fallback: if requested profile doesn't exist, find the first available one
  if (!existsSync(srcProfile)) {
    const fallback = findFirstChromeProfile(chromeDir);
    if (fallback) {
      profileName = fallback;
      srcProfile = join(chromeDir, profileName);
    } else {
      console.log(`  Profile sync: No Chrome profile found, skipping sync`);
      return { synced: false };
    }
  }

  const destDir = getAutomationProfileDir();
  const destProfile = join(destDir, 'Default');
  mkdirSync(destProfile, { recursive: true });

  // Copy Local State (encryption config) from user-data-dir root
  const localStateSrc = join(chromeDir, 'Local State');
  if (existsSync(localStateSrc)) {
    safeCopy(localStateSrc, join(destDir, 'Local State'));
  }

  // Auth-critical items inside the profile directory
  const itemsToCopy = [
    'Cookies',
    'Cookies-journal',
    'Local Storage',
    'IndexedDB',
    'Preferences',
  ];

  for (const item of itemsToCopy) {
    const src = join(srcProfile, item);
    const dest = join(destProfile, item);
    if (!existsSync(src)) continue;

    safeCopyRecursive(src, dest);
  }

  console.log(`  Profile sync: Synced "${profileName}" → ${destDir}`);
  return { synced: true };
}

/**
 * Ensure the automation profile exists (syncing if needed) and return its path.
 */
export async function ensureAutomationProfile(opts?: {
  chromeProfile?: string;
  skipSync?: boolean;
}): Promise<string> {
  const profileDir = getAutomationProfileDir();
  mkdirSync(profileDir, { recursive: true });

  await syncProfile(opts);

  return profileDir;
}

/** Find the first Chrome profile directory (Default, Profile 1, Profile 2, etc.). */
function findFirstChromeProfile(chromeDir: string): string | undefined {
  // Check "Default" first
  if (existsSync(join(chromeDir, 'Default'))) return 'Default';

  // Then look for "Profile N" directories
  try {
    const entries = readdirSync(chromeDir, { withFileTypes: true });
    const profiles = entries
      .filter(e => e.isDirectory() && e.name.startsWith('Profile '))
      .map(e => e.name)
      .sort();
    return profiles[0];
  } catch {
    return undefined;
  }
}

/** Copy a single file, ignoring lock errors. */
function safeCopy(src: string, dest: string): void {
  try {
    copyFileSync(src, dest);
  } catch (err) {
    const code = (err as NodeJS.ErrnoException).code;
    if (code === 'EBUSY' || code === 'EACCES') {
      console.log(`  Profile sync: Skipped locked file ${src}`);
    } else {
      throw err;
    }
  }
}

/** Recursively copy a file or directory, skipping caches and locked files. */
function safeCopyRecursive(src: string, dest: string): void {
  try {
    cpSync(src, dest, {
      recursive: true,
      force: true,
      filter: (source) => {
        const name = source.split('/').pop() ?? '';
        return !SKIP_PATTERNS.includes(name);
      },
    });
  } catch (err) {
    const code = (err as NodeJS.ErrnoException).code;
    if (code === 'EBUSY' || code === 'EACCES') {
      console.log(`  Profile sync: Skipped locked path ${src}`);
    } else {
      throw err;
    }
  }
}
