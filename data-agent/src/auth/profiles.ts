/**
 * Auth Profiles — Discover and manage saved authentication profiles.
 */

import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { homedir } from 'node:os';
import type { AuthProfileMeta } from '../types.js';

const AUTH_DIR = join(homedir(), '.data-agent', 'auth');

/**
 * Discover an auth profile for a domain.
 * Returns the newest profile if multiple exist.
 */
export function discoverAuthProfile(domain: string): {
  storageStatePath: string;
  profileName: string;
} | null {
  const domainDir = join(AUTH_DIR, domain.replace(/[^a-zA-Z0-9.-]/g, '_'));

  if (!existsSync(domainDir)) return null;

  const storageStatePath = join(domainDir, 'storageState.json');
  if (!existsSync(storageStatePath)) return null;

  // Read profile name from meta
  let profileName = 'default';
  try {
    const meta: AuthProfileMeta = JSON.parse(readFileSync(join(domainDir, 'meta.json'), 'utf-8'));
    profileName = meta.profileName;
  } catch { /* use default */ }

  return { storageStatePath, profileName };
}

/**
 * List all auth profiles.
 */
export function listAuthProfiles(): Array<{ domain: string; profileName: string; createdAt?: string }> {
  if (!existsSync(AUTH_DIR)) return [];

  const profiles: Array<{ domain: string; profileName: string; createdAt?: string }> = [];

  for (const entry of readdirSync(AUTH_DIR)) {
    const domainDir = join(AUTH_DIR, entry);
    if (!statSync(domainDir).isDirectory()) continue;

    const storageStatePath = join(domainDir, 'storageState.json');
    if (!existsSync(storageStatePath)) continue;

    let profileName = 'default';
    let createdAt: string | undefined;

    try {
      const meta: AuthProfileMeta = JSON.parse(readFileSync(join(domainDir, 'meta.json'), 'utf-8'));
      profileName = meta.profileName;
      createdAt = meta.createdAt;
    } catch { /* use defaults */ }

    profiles.push({ domain: entry, profileName, createdAt });
  }

  return profiles;
}
