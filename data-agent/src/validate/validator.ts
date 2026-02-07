/**
 * Validator — Execute generated scripts, validate output, provide error feedback.
 *
 * Implements the execute → validate → diagnose → refine → validate loop (up to 5x).
 */

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import type { RunResult, LlmProvider, AnalysisResult } from '../types.js';
import { refineScript } from '../generate/script-generator.js';

const PROJECT_NODE_MODULES = resolve(import.meta.dirname, '../../node_modules');

interface ValidateOptions {
  sessionDir: string;
  llm: LlmProvider;
  maxIterations?: number;
  scriptTimeout?: number;
}

interface ValidateResult {
  success: boolean;
  iterations: number;
  finalExitCode: number;
  outputItemCount: number | null;
}

/**
 * Run the validate-and-refine loop.
 */
export async function validate(options: ValidateOptions): Promise<ValidateResult> {
  const { sessionDir, llm, maxIterations = 5, scriptTimeout = 120_000 } = options;

  // Load analysis and session for refinement context
  let analysis: AnalysisResult | undefined;
  let task = '';
  let url = '';

  try {
    analysis = JSON.parse(readFileSync(join(sessionDir, 'analysis.json'), 'utf-8'));
  } catch { /* may not exist */ }

  try {
    const session = JSON.parse(readFileSync(join(sessionDir, 'session.json'), 'utf-8'));
    task = session.task ?? '';
    url = session.url ?? '';
  } catch { /* may not exist */ }

  let finalExitCode = 1;
  let outputItemCount: number | null = null;

  for (let i = 1; i <= maxIterations; i++) {
    console.log(`  Iteration ${i}/${maxIterations} — Running automation.ts...`);

    const result = executeScript(sessionDir, scriptTimeout);
    const durationStr = (result.durationMs / 1000).toFixed(1);

    if (result.success) {
      console.log(`  Success! output.json has ${result.outputItemCount} items (${durationStr}s)`);
      return {
        success: true,
        iterations: i,
        finalExitCode: result.exitCode,
        outputItemCount: result.outputItemCount,
      };
    }

    const feedback = extractErrorFeedback(result, sessionDir);
    const shortSummary = feedback.split('\n').slice(0, 3).join(' ').slice(0, 120);

    if (result.exitCode !== 0) {
      console.log(`  Failed (exit ${result.exitCode}, ${durationStr}s) — ${shortSummary}`);
    } else {
      const { reason } = validateOutput(sessionDir);
      console.log(`  Exit 0 but ${reason || 'output invalid'} (${durationStr}s)`);
    }

    finalExitCode = result.exitCode;

    // Don't refine after last iteration
    if (i === maxIterations) break;

    // Refine
    console.log('  Refining script...');
    const scriptPath = join(sessionDir, 'automation.ts');
    const currentScript = readFileSync(scriptPath, 'utf-8');

    // Backup current script
    writeFileSync(join(sessionDir, `automation.backup-${Date.now()}.ts`), currentScript, 'utf-8');

    if (analysis) {
      await refineScript({
        sessionDir,
        previousScript: currentScript,
        feedback,
        analysis,
        task,
        url,
        llm,
      });
    }
  }

  return {
    success: false,
    iterations: maxIterations,
    finalExitCode,
    outputItemCount,
  };
}

/**
 * Execute automation.ts in a session directory.
 */
function executeScript(sessionDir: string, timeoutMs: number = 120_000): RunResult {
  const start = Date.now();

  const result = spawnSync('npx', ['tsx', 'automation.ts'], {
    cwd: sessionDir,
    shell: true,
    timeout: timeoutMs,
    encoding: 'utf-8',
    maxBuffer: 10 * 1024 * 1024,
    env: {
      ...process.env,
      NODE_PATH: PROJECT_NODE_MODULES,
    },
  });

  const durationMs = Date.now() - start;
  const exitCode = result.status ?? 1;
  const stdout = (result.stdout ?? '').toString();
  const stderr = (result.stderr ?? '').toString();
  const { valid, itemCount } = validateOutput(sessionDir);

  return {
    success: exitCode === 0 && valid,
    exitCode,
    stdout,
    stderr,
    outputValid: valid,
    outputItemCount: itemCount ?? null,
    durationMs,
  };
}

/**
 * Check whether output.json exists, is valid JSON, and is non-empty.
 */
function validateOutput(sessionDir: string): { valid: boolean; itemCount?: number; reason?: string } {
  const outputPath = join(sessionDir, 'output.json');

  if (!existsSync(outputPath)) return { valid: false, reason: 'output.json does not exist' };

  let raw: string;
  try { raw = readFileSync(outputPath, 'utf-8'); } catch { return { valid: false, reason: 'Could not read output.json' }; }

  if (!raw.trim()) return { valid: false, reason: 'output.json is empty' };

  let parsed: unknown;
  try { parsed = JSON.parse(raw); } catch { return { valid: false, reason: 'output.json is not valid JSON' }; }

  if (parsed === null) return { valid: false, reason: 'output.json is null' };

  if (Array.isArray(parsed)) {
    if (parsed.length === 0) return { valid: false, itemCount: 0, reason: 'output.json is an empty array []' };
    return { valid: true, itemCount: parsed.length };
  }

  if (typeof parsed === 'object') {
    const keys = Object.keys(parsed as Record<string, unknown>);
    if (keys.length === 0) return { valid: false, itemCount: 0, reason: 'output.json is an empty object {}' };
    return { valid: true, itemCount: keys.length };
  }

  return { valid: true, itemCount: 1 };
}

/**
 * Parse `[resp] STATUS URL` lines from stdout.
 */
function parseResponseLog(stdout: string): Array<{ status: number; url: string; count: number }> {
  const counts = new Map<string, { status: number; url: string; count: number }>();

  for (const line of stdout.split('\n')) {
    const m = line.match(/\[resp\]\s+(\d{3})\s+(.+)/);
    if (!m) continue;
    const status = parseInt(m[1], 10);
    const respUrl = m[2].trim();
    const key = `${status} ${respUrl}`;
    const existing = counts.get(key);
    if (existing) existing.count++;
    else counts.set(key, { status, url: respUrl, count: 1 });
  }

  return Array.from(counts.values());
}

/**
 * Pattern-match execution output to produce actionable feedback.
 */
function extractErrorFeedback(result: RunResult, sessionDir: string): string {
  const combined = result.stderr + '\n' + result.stdout;
  const classifications: string[] = [];
  const responseLog = parseResponseLog(result.stdout);

  let targetDomain = '';
  try {
    const session = JSON.parse(readFileSync(join(sessionDir, 'session.json'), 'utf-8'));
    targetDomain = session.targetDomain || '';
  } catch { /* ok */ }

  const isTargetApiUrl = (url: string): boolean => {
    if (!targetDomain) return true;
    if (!url.includes(targetDomain)) return false;
    const ignorePaths = ['/telemetry', '/analytics', '/tracking', '/ces/', '/log/', '/pixel'];
    return !ignorePaths.some(p => url.includes(p));
  };

  const apiErrors = responseLog.filter(e => isTargetApiUrl(e.url));
  const has403 = apiErrors.some(e => e.status === 403);
  const has401 = apiErrors.some(e => e.status === 401);
  const has429 = apiErrors.some(e => e.status === 429);

  if (has403) {
    classifications.push(
      'HTTP 403 Forbidden on target API. Likely CSRF/auth/sentinel issue. ' +
      'Add `&& r.status() === 200` to waitForResponse predicate. ' +
      'Try waitForLoadState("networkidle") before API calls.',
    );
  }
  if (has401) {
    classifications.push(
      'HTTP 401 Unauthorized. Verify storageState.json or auth.json is loaded before navigation.',
    );
  }
  if (has429) {
    classifications.push('HTTP 429 Too Many Requests. Add delays between requests.');
  }

  if (/Unexpected token\s*[<']/.test(combined) || /SyntaxError.*JSON/.test(combined)) {
    classifications.push(
      'JSON parse error — API returned HTML instead of JSON. Check auth and URL correctness.',
    );
  }

  if (/waitForResponse.*timed?\s*out/i.test(combined) || /Timeout.*exceeded/i.test(combined)) {
    classifications.push(
      'waitForResponse or navigation timed out. The expected API call may not be firing.',
    );
  }

  if (/waiting for (selector|locator)/i.test(combined) || /strict mode violation/i.test(combined)) {
    classifications.push(
      'Selector/locator not found or matched multiple elements. Use more specific selectors.',
    );
  }

  const targetApi200 = apiErrors.some(e => e.status === 200);
  if (targetApi200 && !result.outputValid) {
    classifications.push(
      'Target API returned 200 but data not captured. Set up listener BEFORE navigation.',
    );
  } else if (result.exitCode === 0 && !result.outputValid) {
    classifications.push(
      'Script exited 0 but output.json is missing/empty. Check interception logic.',
    );
  }

  let feedback: string;
  if (classifications.length > 0) {
    feedback = 'AUTOMATED ERROR ANALYSIS:\n' + classifications.map((c, i) => `${i + 1}. ${c}`).join('\n');
  } else {
    feedback = 'Script failed with no recognized error pattern.';
  }

  if (responseLog.length > 0) {
    const logLines = responseLog.map(e => `  ${e.status} ${e.url.slice(0, 120)} (x${e.count})`);
    feedback += `\n\nRESPONSE LOG SUMMARY:\n${logLines.join('\n')}`;
  }

  const outputPath = join(sessionDir, 'output.json');
  if (existsSync(outputPath)) {
    try {
      const raw = readFileSync(outputPath, 'utf-8').trim();
      if (raw === '[]' || raw === '{}') {
        feedback += `\n\nOUTPUT.JSON: exists but empty (${raw})`;
      } else if (raw.length > 0) {
        const parsed = JSON.parse(raw);
        const count = Array.isArray(parsed) ? parsed.length : Object.keys(parsed).length;
        feedback += `\n\nOUTPUT.JSON: ${count} items, ${raw.length} bytes`;
      }
    } catch {
      feedback += '\n\nOUTPUT.JSON: exists but not valid JSON';
    }
  } else {
    feedback += '\n\nOUTPUT.JSON: does not exist';
  }

  const stderrTail = result.stderr.split('\n').slice(-50).join('\n');
  const stdoutTail = result.stdout.split('\n').slice(-80).join('\n');
  feedback += `\n\nRAW STDERR (last 50 lines):\n${stderrTail}\n\nRAW STDOUT (last 80 lines):\n${stdoutTail}`;

  if (feedback.length > 4000) feedback = feedback.slice(0, 3997) + '...';
  return feedback;
}
