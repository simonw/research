/**
 * Standardized artifacts bundle — every run produces a stable set of files.
 *
 * Formalizes the session directory contract so artifacts are debuggable
 * and reproducible without re-running the LLM.
 *
 * Inspired by Skyvern's service-boundary pattern (treat browser execution
 * as a service that produces artifacts):
 * @see https://github.com/Skyvern-AI/skyvern/blob/main/skyvern/forge/agent.py
 * Service boundary: isolate flaky browser tasks with queues/timeouts/artifacts.
 */

import { existsSync, mkdirSync, writeFileSync, readFileSync, appendFileSync } from 'node:fs';
import { join } from 'node:path';
import type { PageStats } from './page-stats.js';

/**
 * Standard artifact filenames within a session directory.
 */
export const ARTIFACT_FILES = {
  HAR: 'recording.har',
  SESSION: 'session.json',
  ANALYSIS: 'analysis.json',
  AUTOMATION: 'automation.ts',
  OUTPUT: 'output.json',
  RUN_LOG: 'run.log',
  PAGE_STATS: 'page-stats.json',
} as const;

export type ArtifactName = keyof typeof ARTIFACT_FILES;

export interface ArtifactBundle {
  sessionDir: string;
  /** Whether each artifact file exists. */
  files: Record<string, boolean>;
}

/**
 * Ensure a session directory exists and return an artifact bundle descriptor.
 */
export function initArtifactBundle(sessionDir: string): ArtifactBundle {
  mkdirSync(sessionDir, { recursive: true });

  const files: Record<string, boolean> = {};
  for (const [, filename] of Object.entries(ARTIFACT_FILES)) {
    files[filename] = existsSync(join(sessionDir, filename));
  }

  return { sessionDir, files };
}

/**
 * Write an artifact file to the session directory.
 */
export function writeArtifact(
  sessionDir: string,
  name: string,
  content: string | object,
): void {
  const data = typeof content === 'string' ? content : JSON.stringify(content, null, 2);
  writeFileSync(join(sessionDir, name), data, 'utf-8');
}

/**
 * Read an artifact file from the session directory.
 * Returns null if the file doesn't exist.
 */
export function readArtifact(sessionDir: string, name: string): string | null {
  const path = join(sessionDir, name);
  if (!existsSync(path)) return null;
  return readFileSync(path, 'utf-8');
}

/**
 * Append a line to the run log.
 */
export function appendRunLog(sessionDir: string, line: string): void {
  const logPath = join(sessionDir, ARTIFACT_FILES.RUN_LOG);
  appendFileSync(logPath, `[${new Date().toISOString()}] ${line}\n`, 'utf-8');
}

/**
 * Record page stats snapshot to the page-stats.json artifact.
 * Appends to an array of snapshots (one per step).
 */
export function recordPageStats(sessionDir: string, step: number, stats: PageStats): void {
  const statsPath = join(sessionDir, ARTIFACT_FILES.PAGE_STATS);
  let existing: Array<{ step: number; stats: PageStats }> = [];

  if (existsSync(statsPath)) {
    try {
      existing = JSON.parse(readFileSync(statsPath, 'utf-8'));
    } catch { /* corrupt file, start fresh */ }
  }

  existing.push({ step, stats });
  writeFileSync(statsPath, JSON.stringify(existing, null, 2), 'utf-8');
}

/**
 * Inventory a session directory — check which artifacts exist and return summary.
 */
export function inventoryBundle(sessionDir: string): ArtifactBundle {
  const files: Record<string, boolean> = {};
  for (const [, filename] of Object.entries(ARTIFACT_FILES)) {
    files[filename] = existsSync(join(sessionDir, filename));
  }
  return { sessionDir, files };
}
