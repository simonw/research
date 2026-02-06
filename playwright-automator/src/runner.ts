/**
 * Script executor + self-correcting agent loop.
 *
 * Runs the generated automation.ts, validates output, extracts error
 * feedback, and refines via Gemini until the script produces valid output.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { refineScript } from './gemini.js';
import type { RecordingSession } from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface RunResult {
  success: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  outputValid: boolean;
  outputItemCount: number | null;
  durationMs: number;
}

export interface IterationRecord {
  iteration: number;
  exitCode: number;
  errorSummary: string;
  outputValid: boolean;
  durationMs: number;
}

export interface AgentLoopOptions {
  sessionDir: string;
  apiKey: string;
  maxIterations?: number;
  scriptTimeout?: number;
}

export interface AgentLoopResult {
  success: boolean;
  iterations: number;
  finalExitCode: number;
  outputItemCount: number | null;
  history: IterationRecord[];
}

// ---------------------------------------------------------------------------
// executeScript
// ---------------------------------------------------------------------------

/**
 * Run automation.ts inside `sessionDir` and return structured results.
 */
export function executeScript(sessionDir: string, timeoutMs: number = 120_000): RunResult {
  const start = Date.now();

  const result = spawnSync('npx', ['tsx', 'automation.ts'], {
    cwd: sessionDir,
    shell: true,
    timeout: timeoutMs,
    encoding: 'utf-8',
    maxBuffer: 10 * 1024 * 1024,
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

// ---------------------------------------------------------------------------
// validateOutput
// ---------------------------------------------------------------------------

/**
 * Check whether output.json exists, is valid JSON, and is non-empty.
 */
export function validateOutput(sessionDir: string): { valid: boolean; itemCount?: number; reason?: string } {
  const outputPath = join(sessionDir, 'output.json');

  if (!existsSync(outputPath)) {
    return { valid: false, reason: 'output.json does not exist' };
  }

  let raw: string;
  try {
    raw = readFileSync(outputPath, 'utf-8');
  } catch {
    return { valid: false, reason: 'Could not read output.json' };
  }

  if (!raw.trim()) {
    return { valid: false, reason: 'output.json is empty' };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { valid: false, reason: 'output.json is not valid JSON' };
  }

  if (parsed === null) {
    return { valid: false, reason: 'output.json is null' };
  }

  if (Array.isArray(parsed)) {
    if (parsed.length === 0) {
      return { valid: false, itemCount: 0, reason: 'output.json is an empty array []' };
    }
    return { valid: true, itemCount: parsed.length };
  }

  if (typeof parsed === 'object') {
    const keys = Object.keys(parsed as Record<string, unknown>);
    if (keys.length === 0) {
      return { valid: false, itemCount: 0, reason: 'output.json is an empty object {}' };
    }
    return { valid: true, itemCount: keys.length };
  }

  // Primitive value — technically valid JSON but unusual
  return { valid: true, itemCount: 1 };
}

// ---------------------------------------------------------------------------
// parseResponseLog
// ---------------------------------------------------------------------------

export interface ResponseLogEntry {
  status: number;
  url: string;
  count: number;
}

/**
 * Parse `[resp] STATUS URL` lines from stdout into a structured summary.
 * The automation scripts log these via the diagnostic logger.
 */
export function parseResponseLog(stdout: string): ResponseLogEntry[] {
  const counts = new Map<string, { status: number; url: string; count: number }>();

  for (const line of stdout.split('\n')) {
    const m = line.match(/\[resp\]\s+(\d{3})\s+(.+)/);
    if (!m) continue;
    const status = parseInt(m[1], 10);
    const url = m[2].trim();
    const key = `${status} ${url}`;
    const existing = counts.get(key);
    if (existing) {
      existing.count++;
    } else {
      counts.set(key, { status, url, count: 1 });
    }
  }

  return Array.from(counts.values());
}

// ---------------------------------------------------------------------------
// extractErrorFeedback
// ---------------------------------------------------------------------------

/**
 * Pattern-match execution output to produce actionable feedback for Gemini.
 *
 * Reads session.json from `sessionDir` to scope HTTP-error classifiers to
 * the target domain's API paths (ignoring analytics/telemetry 403s).
 */
export function extractErrorFeedback(result: RunResult, sessionDir: string): string {
  const combined = result.stderr + '\n' + result.stdout;
  const classifications: string[] = [];

  // Parse structured response log from stdout
  const responseLog = parseResponseLog(result.stdout);

  // Read target domain from session.json so we only flag errors on relevant endpoints
  let targetDomain = '';
  try {
    const session = JSON.parse(readFileSync(join(sessionDir, 'session.json'), 'utf-8'));
    targetDomain = session.targetDomain || '';
  } catch { /* session.json may not exist yet */ }

  // Helper: does a URL belong to the target domain's API (not analytics/telemetry)?
  const isTargetApiUrl = (url: string): boolean => {
    if (!targetDomain) return true; // no domain info → be conservative, flag everything
    if (!url.includes(targetDomain)) return false;
    // Exclude common non-API paths
    const ignorePaths = ['/telemetry', '/analytics', '/tracking', '/ces/', '/log/', '/pixel'];
    return !ignorePaths.some((p) => url.includes(p));
  };

  // HTTP errors — only on target domain API endpoints
  const apiErrors = responseLog.filter((e) => isTargetApiUrl(e.url));
  const has403 = apiErrors.some((e) => e.status === 403);
  const has401 = apiErrors.some((e) => e.status === 401);
  const has429 = apiErrors.some((e) => e.status === 429);

  if (has403) {
    classifications.push(
      'HTTP 403 Forbidden on target API endpoint. The API is rejecting the request — likely a CSRF/auth/sentinel issue. ' +
      'Common causes: (a) the waitForResponse caught a 403 response — add `&& r.status() === 200` to the predicate to only catch successful responses, ' +
      '(b) page.evaluate(fetch) needs ALL auth headers from auth.json (not just cookies), ' +
      '(c) the site may require a sentinel/challenge token flow before API calls work. ' +
      'Try: wait for the page to fully load (waitForLoadState("networkidle")), then use page.evaluate(fetch) with all auth headers from authData.authHeaders.',
    );
  }
  if (has401) {
    classifications.push(
      'HTTP 401 Unauthorized on target API endpoint. Auth cookies/tokens may not be loading correctly. ' +
      'Verify that storageState.json or auth.json is being loaded before navigation. ' +
      'Ensure context.addCookies() is called with playwrightCookies (not the flat cookies object).',
    );
  }
  if (has429) {
    classifications.push(
      'HTTP 429 Too Many Requests on target API endpoint. Add delays between requests and reduce concurrency.',
    );
  }

  // JSON parse error (API returned HTML instead of JSON)
  if (/Unexpected token\s*[<']/.test(combined) || /SyntaxError.*JSON/.test(combined)) {
    classifications.push(
      'JSON parse error — the API likely returned HTML instead of JSON. This usually means the request was ' +
      'redirected to a login page or error page. Ensure auth is loaded before making the request, and check ' +
      'that the URL is correct. Consider using page.evaluate(() => fetch(...)) for in-page requests.',
    );
  }

  // Timeout
  if (/waitForResponse.*timed?\s*out/i.test(combined) || /Timeout.*exceeded/i.test(combined)) {
    classifications.push(
      'waitForResponse or navigation timed out. The expected API call may not be firing. ' +
      'Consider navigating differently or falling back to direct page.evaluate(() => fetch(...)) calls ' +
      'using the request templates from the HAR.',
    );
  }

  // Selector not found
  if (/waiting for (selector|locator)/i.test(combined) || /strict mode violation/i.test(combined)) {
    classifications.push(
      'Selector/locator not found or matched multiple elements. Use more specific selectors, ' +
      'add waitForSelector() with a timeout, or prefer API interception over DOM scraping.',
    );
  }

  // Target API returned 200 but data wasn't captured
  const targetApi200 = apiErrors.some((e) => e.status === 200);
  if (targetApi200 && !result.outputValid) {
    classifications.push(
      'The target API responded with 200 but data was not captured in output.json. ' +
      'The response may have fired BEFORE the listener was set up. ' +
      'Set up page.on(\'response\') or waitForResponse() BEFORE navigating to the page. ' +
      'Alternatively, use page.evaluate(() => fetch(...)) to make the request directly after page load.',
    );
  } else if (result.exitCode === 0 && !result.outputValid) {
    // Exit 0 but empty output (no response log to analyze)
    classifications.push(
      'Script exited successfully but output.json is missing or empty. ' +
      'The interception logic may not be capturing data. Verify that the response listener is set up ' +
      'before navigation, and that the correct URL pattern is being matched. ' +
      'Log the URLs of all intercepted responses to diagnose which calls are actually firing.',
    );
  }

  // Build feedback string
  let feedback: string;
  if (classifications.length > 0) {
    feedback = 'AUTOMATED ERROR ANALYSIS:\n' + classifications.map((c, i) => `${i + 1}. ${c}`).join('\n');
  } else {
    feedback = 'Script failed with no recognized error pattern.';
  }

  // Append response log summary if available
  if (responseLog.length > 0) {
    const logLines = responseLog.map(
      (e) => `  ${e.status} ${e.url.slice(0, 120)} (x${e.count})`,
    );
    feedback += `\n\nRESPONSE LOG SUMMARY (${responseLog.length} unique endpoints):\n${logLines.join('\n')}`;
  }

  // Append output.json status
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

  // Append raw error details
  const stderrTail = result.stderr.split('\n').slice(-50).join('\n');
  const stdoutTail = result.stdout.split('\n').slice(-80).join('\n');
  const rawDetails = `\n\nRAW STDERR (last 50 lines):\n${stderrTail}\n\nRAW STDOUT (last 80 lines):\n${stdoutTail}`;

  feedback += rawDetails;

  // Truncate to 4000 chars
  if (feedback.length > 4000) {
    feedback = feedback.slice(0, 3997) + '...';
  }

  return feedback;
}

// ---------------------------------------------------------------------------
// runAgentLoop
// ---------------------------------------------------------------------------

/**
 * Execute → validate → refine loop until the script produces valid output.
 */
export async function runAgentLoop(options: AgentLoopOptions): Promise<AgentLoopResult> {
  const {
    sessionDir,
    apiKey,
    maxIterations = 5,
    scriptTimeout = 120_000,
  } = options;

  const sessionPath = join(sessionDir, 'session.json');
  const session: RecordingSession = JSON.parse(readFileSync(sessionPath, 'utf-8'));
  const scriptPath = join(sessionDir, 'automation.ts');

  const history: IterationRecord[] = [];
  let finalExitCode = 1;
  let outputItemCount: number | null = null;
  let success = false;

  for (let i = 1; i <= maxIterations; i++) {
    console.log(`\n  Iteration ${i}/${maxIterations} — Running automation.ts...`);

    const result = executeScript(sessionDir, scriptTimeout);
    const durationStr = (result.durationMs / 1000).toFixed(1);

    if (result.success) {
      console.log(`  ✅ Success! output.json has ${result.outputItemCount} items (${durationStr}s)`);

      history.push({
        iteration: i,
        exitCode: result.exitCode,
        errorSummary: '',
        outputValid: true,
        durationMs: result.durationMs,
      });

      success = true;
      finalExitCode = result.exitCode;
      outputItemCount = result.outputItemCount;
      break;
    }

    // Build error summary for the history record
    const feedback = extractErrorFeedback(result, sessionDir);
    const shortSummary = feedback.split('\n').slice(0, 3).join(' ').slice(0, 120);

    if (result.exitCode !== 0) {
      console.log(`  ❌ Failed (exit ${result.exitCode}, ${durationStr}s) — ${shortSummary}`);
    } else {
      const { reason } = validateOutput(sessionDir);
      console.log(`  ⚠️  Exit 0 but ${reason || 'output invalid'} (${durationStr}s)`);
    }

    history.push({
      iteration: i,
      exitCode: result.exitCode,
      errorSummary: shortSummary,
      outputValid: false,
      durationMs: result.durationMs,
    });

    finalExitCode = result.exitCode;

    // Don't refine after the last iteration — just report the failure
    if (i === maxIterations) break;

    // Refine
    console.log('  Refining with Gemini...');

    const currentScript = readFileSync(scriptPath, 'utf-8');
    const refined = await refineScript(session, currentScript, feedback, apiKey);

    // Backup current script
    const backupPath = join(sessionDir, `automation.backup-${Date.now()}.ts`);
    writeFileSync(backupPath, currentScript, 'utf-8');
    writeFileSync(scriptPath, refined.script, 'utf-8');
  }

  // Write agent loop result to session dir
  const loopResult: AgentLoopResult = {
    success,
    iterations: history.length,
    finalExitCode,
    outputItemCount,
    history,
  };

  writeFileSync(
    join(sessionDir, 'agent-loop-result.json'),
    JSON.stringify(loopResult, null, 2),
    'utf-8',
  );

  return loopResult;
}
