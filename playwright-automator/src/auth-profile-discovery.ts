/**
 * Discover an auth profile automatically for a given URL.
 *
 * Convention: auth profiles live under:
 *   auth-profiles/<domain>/<profileName>/storageState.json
 *
 * Selection heuristic:
 * - prefer newest meta.json.updatedAt/createdAt if present
 * - otherwise newest storageState.json mtime
 */

import { readdirSync, statSync, existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

interface Meta {
  createdAt?: string;
  updatedAt?: string;
}

function safeStat(path: string) {
  try {
    return statSync(path);
  } catch {
    return null;
  }
}

function safeReadJson(path: string): any | null {
  try {
    return JSON.parse(readFileSync(path, 'utf-8'));
  } catch {
    return null;
  }
}

function bestTimestamp(meta: Meta | null, storageStatePath: string): number {
  const candidates: number[] = [];
  if (meta?.updatedAt) candidates.push(Date.parse(meta.updatedAt));
  if (meta?.createdAt) candidates.push(Date.parse(meta.createdAt));
  const st = safeStat(storageStatePath);
  if (st) candidates.push(st.mtimeMs);
  const t = Math.max(...candidates.filter((x) => Number.isFinite(x)), 0);
  return t;
}

export function discoverAuthProfileForUrl(url: string, cwd = process.cwd()): { storageStatePath: string; profileName: string } | null {
  let domain = '';
  try {
    domain = new URL(url).hostname;
  } catch {
    return null;
  }

  const domainDir = join(cwd, 'auth-profiles', domain);
  if (!existsSync(domainDir)) return null;

  const profiles = readdirSync(domainDir, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name);

  let best: { storageStatePath: string; profileName: string; ts: number } | null = null;
  for (const profileName of profiles) {
    const dir = join(domainDir, profileName);
    const storageStatePath = join(dir, 'storageState.json');
    if (!existsSync(storageStatePath)) continue;

    const meta = safeReadJson(join(dir, 'meta.json')) as Meta | null;
    const ts = bestTimestamp(meta, storageStatePath);

    if (!best || ts > best.ts) {
      best = { storageStatePath, profileName, ts };
    }
  }

  return best ? { storageStatePath: best.storageStatePath, profileName: best.profileName } : null;
}
