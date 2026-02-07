/**
 * Registry — Global extractor registry for replay.
 *
 * Stores published extractors at ~/.data-agent/registry.json.
 * Each entry maps a domain+task to a saved automation.ts script.
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync, copyFileSync } from 'node:fs';
import { join } from 'node:path';
import { homedir } from 'node:os';
import type { Registry, RegistryEntry } from '../types.js';

const REGISTRY_DIR = join(homedir(), '.data-agent');
const REGISTRY_PATH = join(REGISTRY_DIR, 'registry.json');
const EXTRACTORS_DIR = join(REGISTRY_DIR, 'extractors');

function ensureRegistryDir(): void {
  if (!existsSync(REGISTRY_DIR)) mkdirSync(REGISTRY_DIR, { recursive: true });
  if (!existsSync(EXTRACTORS_DIR)) mkdirSync(EXTRACTORS_DIR, { recursive: true });
}

function loadRegistry(): Registry {
  ensureRegistryDir();
  if (!existsSync(REGISTRY_PATH)) return { version: 1, entries: [] };
  try {
    return JSON.parse(readFileSync(REGISTRY_PATH, 'utf-8'));
  } catch {
    return { version: 1, entries: [] };
  }
}

function saveRegistry(registry: Registry): void {
  ensureRegistryDir();
  writeFileSync(REGISTRY_PATH, JSON.stringify(registry, null, 2));
}

/**
 * Publish an extractor to the global registry.
 */
export async function publishToRegistry(opts: {
  domain: string;
  task: string;
  sessionDir: string;
}): Promise<void> {
  const { domain, task, sessionDir } = opts;
  const registry = loadRegistry();

  const scriptSrc = join(sessionDir, 'automation.ts');
  if (!existsSync(scriptSrc)) {
    throw new Error(`No automation.ts found in ${sessionDir}`);
  }

  // Create extractor directory
  const extractorDir = join(EXTRACTORS_DIR, domain.replace(/[^a-zA-Z0-9.-]/g, '_'));
  if (!existsSync(extractorDir)) mkdirSync(extractorDir, { recursive: true });

  // Copy script
  const scriptDest = join(extractorDir, 'automation.ts');
  copyFileSync(scriptSrc, scriptDest);

  // Update registry
  const existingIdx = registry.entries.findIndex(e => e.domain === domain);
  const entry: RegistryEntry = {
    domain,
    task,
    scriptPath: scriptDest,
    createdAt: new Date().toISOString(),
    runCount: 0,
  };

  if (existingIdx >= 0) {
    registry.entries[existingIdx] = entry;
  } else {
    registry.entries.push(entry);
  }

  saveRegistry(registry);
}

/**
 * Find an extractor for a domain.
 */
export function findExtractor(domain: string): RegistryEntry | undefined {
  const registry = loadRegistry();
  return registry.entries.find(e => e.domain === domain);
}

/**
 * List all registered extractors.
 */
export async function listRegistry(): Promise<void> {
  const registry = loadRegistry();

  if (registry.entries.length === 0) {
    console.log('No extractors registered yet.');
    console.log('Run: data-agent "description" to create one.');
    return;
  }

  console.log(`\nRegistered extractors (${registry.entries.length}):\n`);
  for (const entry of registry.entries) {
    const lastRun = entry.lastRunAt ? ` (last run: ${entry.lastRunAt})` : '';
    const status = entry.lastRunSuccess === true ? ' [OK]' : entry.lastRunSuccess === false ? ' [FAIL]' : '';
    console.log(`  ${entry.domain}${status}${lastRun}`);
    console.log(`    Task: ${entry.task}`);
    console.log(`    Script: ${entry.scriptPath}`);
    console.log(`    Runs: ${entry.runCount}`);
    console.log('');
  }
}

/**
 * Update registry after a replay run.
 */
export function updateRegistryAfterRun(domain: string, success: boolean, itemCount?: number): void {
  const registry = loadRegistry();
  const entry = registry.entries.find(e => e.domain === domain);
  if (!entry) return;

  entry.lastRunAt = new Date().toISOString();
  entry.runCount++;
  entry.lastRunSuccess = success;
  if (itemCount !== undefined) entry.lastRunItemCount = itemCount;

  saveRegistry(registry);
}
