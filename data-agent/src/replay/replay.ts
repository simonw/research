/**
 * Replay — Run a saved extractor script without LLM.
 *
 * Pure Playwright execution of a previously generated automation.ts.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { findExtractor, updateRegistryAfterRun } from './registry.js';

/** Resolve data-agent's node_modules so replay scripts can find dependencies. */
const PROJECT_NODE_MODULES = resolve(import.meta.dirname, '../../node_modules');

/**
 * Replay a saved extractor for a domain.
 */
export async function replay(domain: string): Promise<void> {
  const entry = findExtractor(domain);

  if (!entry) {
    console.error(`No extractor found for "${domain}".`);
    console.error('Run: data-agent list — to see available extractors');
    console.error('Run: data-agent "description" — to create a new one');
    process.exit(1);
  }

  if (!existsSync(entry.scriptPath)) {
    console.error(`Script not found: ${entry.scriptPath}`);
    process.exit(1);
  }

  const scriptDir = dirname(entry.scriptPath);
  console.log(`Replaying extractor for ${domain}...`);
  console.log(`  Script: ${entry.scriptPath}`);
  console.log(`  Task: ${entry.task}`);

  const start = Date.now();

  const result = spawnSync('npx', ['tsx', 'automation.ts'], {
    cwd: scriptDir,
    shell: true,
    timeout: 120_000,
    encoding: 'utf-8',
    maxBuffer: 10 * 1024 * 1024,
    stdio: ['inherit', 'pipe', 'pipe'],
    env: {
      ...process.env,
      NODE_PATH: PROJECT_NODE_MODULES,
    },
  });

  const durationMs = Date.now() - start;
  const durationStr = (durationMs / 1000).toFixed(1);

  if (result.stdout) console.log(result.stdout);
  if (result.stderr) console.error(result.stderr);

  const exitCode = result.status ?? 1;

  // Check output.json
  const outputPath = join(scriptDir, 'output.json');
  let itemCount: number | undefined;

  if (existsSync(outputPath)) {
    try {
      const raw = readFileSync(outputPath, 'utf-8');
      const parsed = JSON.parse(raw);
      itemCount = Array.isArray(parsed) ? parsed.length : Object.keys(parsed).length;
    } catch { /* invalid json */ }
  }

  const success = exitCode === 0 && itemCount !== undefined && itemCount > 0;

  // Update registry
  updateRegistryAfterRun(domain, success, itemCount);

  if (success) {
    console.log(`\nReplay complete (${durationStr}s) — ${itemCount} items → ${outputPath}`);
  } else {
    console.error(`\nReplay failed (exit ${exitCode}, ${durationStr}s)`);
    process.exit(exitCode || 1);
  }
}
