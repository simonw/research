/**
 * Auth profiles: reusable Playwright storage state for a domain.
 */

import { mkdirSync, writeFileSync, existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

export interface AuthProfileMeta {
  profileName: string;
  domain: string;
  createdAt: string;
  updatedAt?: string;
  notes?: string;
}

export function getAuthProfilesRoot(cwd = process.cwd()): string {
  return join(cwd, 'auth-profiles');
}

export function getAuthProfileDir(domain: string, profileName: string, cwd = process.cwd()): string {
  return join(getAuthProfilesRoot(cwd), domain, profileName);
}

export function getAuthProfilePaths(domain: string, profileName: string, cwd = process.cwd()): {
  dir: string;
  storageStatePath: string;
  metaPath: string;
} {
  const dir = getAuthProfileDir(domain, profileName, cwd);
  return {
    dir,
    storageStatePath: join(dir, 'storageState.json'),
    metaPath: join(dir, 'meta.json'),
  };
}

export function writeAuthProfileMeta(meta: AuthProfileMeta, cwd = process.cwd()) {
  const paths = getAuthProfilePaths(meta.domain, meta.profileName, cwd);
  mkdirSync(paths.dir, { recursive: true });
  writeFileSync(paths.metaPath, JSON.stringify(meta, null, 2), 'utf-8');
}

export function readAuthProfileMeta(domain: string, profileName: string, cwd = process.cwd()): AuthProfileMeta | null {
  const { metaPath } = getAuthProfilePaths(domain, profileName, cwd);
  if (!existsSync(metaPath)) return null;
  try {
    return JSON.parse(readFileSync(metaPath, 'utf-8'));
  } catch {
    return null;
  }
}
