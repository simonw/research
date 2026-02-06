#!/usr/bin/env node
/**
 * Playwright Automator — Record browser actions and generate automation scripts.
 *
 * Usage:
 *   npx tsx src/index.ts                    # Interactive mode
 *   npx tsx src/index.ts --url <url> --desc <description> --key <gemini-api-key>
 */

import 'dotenv/config';
import { mkdirSync, existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { startRecording } from './recorder.js';
import { generateScript, refineScript } from './gemini.js';
import { captureLoginStorageState } from './login.js';
import { discoverAuthProfileForUrl } from './auth-profile-discovery.js';
import { runAgentLoop } from './runner.js';
import type { RecordingSession } from './types.js';

const DEFAULT_OUTPUT_DIR = join(process.cwd(), 'runs');

function createReadlineInterface() {
  return createInterface({
    input: process.stdin,
    output: process.stdout,
  });
}

function question(rl: ReturnType<typeof createReadlineInterface>, prompt: string): Promise<string> {
  return new Promise((resolve) => {
    rl.question(prompt, (answer) => {
      resolve(answer.trim());
    });
  });
}

function parseArgs(): {
  command?: string;
  url?: string;
  description?: string;
  apiKey?: string;
  outputDir?: string;
  run?: string;
  feedback?: string;
  headless?: boolean;
  skipGenerate?: boolean;
  authProfile?: string;
  profileName?: string;
  notes?: string;
  maxIterations?: number;
  scriptTimeout?: number;
} {
  const args = process.argv.slice(2);
  const result: Record<string, string | boolean | number> = {};

  // subcommand support:
  //   `pwa login --url ...`
  //   `pwa record --url ... --desc ...`
  //   `pwa refine --run <dir> --feedback ...`
  if (args[0] && !args[0].startsWith('-')) {
    result.command = args[0];
    args.shift();
  }

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case '--url':
      case '-u':
        result.url = args[++i];
        break;
      case '--desc':
      case '--description':
      case '-d':
        result.description = args[++i];
        break;
      case '--key':
      case '--api-key':
      case '-k':
        result.apiKey = args[++i];
        break;
      case '--output':
      case '-o':
        result.outputDir = args[++i];
        break;
      case '--refine':
      case '-r':
        console.error('❌ The --refine flag is no longer supported. Use: pwa refine --run <dir> --feedback "..."');
        process.exit(1);
      case '--run':
        // Preferred: `refine --run <dir>`
        result.run = args[++i];
        break;
      case '--feedback':
      case '-f':
        result.feedback = args[++i];
        break;
      case '--headless':
        result.headless = true;
        break;
      case '--skip-generate':
        result.skipGenerate = true;
        break;
      case '--auth-profile':
        result.authProfile = args[++i];
        break;
      case '--profile':
        result.profileName = args[++i];
        break;
      case '--notes':
        result.notes = args[++i];
        break;
      case '--max-iterations':
      case '-n':
        result.maxIterations = parseInt(args[++i], 10);
        break;
      case '--script-timeout':
        result.scriptTimeout = parseInt(args[++i], 10);
        break;
      case '--help':
      case '-h':
        printHelp();
        process.exit(0);
    }
  }

  return result as any;
}

function printHelp() {
  console.log(`
╔══════════════════════════════════════════════════╗
║           Playwright Automator v1.0              ║
╚══════════════════════════════════════════════════╝

Record browser actions and generate Playwright scripts using AI.

USAGE:
  pwa [command] [options]

COMMANDS:
  record               Record + generate + validate (auto-refines until script works)
  run                  Re-run validation loop on an existing session
  login                Capture reusable auth storageState (headed; supports 2FA)
  refine               Refine an existing run's automation.ts using feedback

OPTIONS:
  --url, -u <url>          Target URL to automate
  --desc, -d <description> What you want to automate
  --key, -k <key>          Gemini API key (or set GEMINI_API_KEY env)
  --output, -o <dir>       Output directory (default: ./runs)
  --headless               Run browser in headless mode
  --skip-generate          Only record, don't generate script
  --auth-profile <path>    Load a Playwright storageState.json before recording (auth replay). If omitted, pwa will auto-pick the newest auth profile for the URL's domain if one exists.
  --run <dir>              (run/refine) session directory
  --max-iterations, -n N   Max validation iterations (default: 5)
  --script-timeout <ms>    Script execution timeout in ms (default: 120000)
  --profile <name>         (login cmd) profile name (default: default)
  --notes <text>           (login cmd) notes for this auth profile
  --feedback, -f <text>    Feedback for refinement
  --help, -h               Show this help

WORKFLOW:
  1. Run the tool and provide a URL + task description
  2. A browser opens — perform the task manually
  3. Close the browser or press ENTER when done to stop recording
  4. The tool captures HAR traffic, actions, and cookies
  5. Gemini AI generates a Playwright script
  6. The script is automatically run and validated
  7. If it fails, Gemini refines it (up to N iterations)

EXAMPLES:
  # Interactive mode
  pwa

  # Record (explicit)
  pwa record --url https://example.com --desc "Scrape all articles"

  # Capture login state (for sites with 2FA)
  pwa login --url https://example.com/login --profile default

  # Record using an existing auth profile (storageState.json)
  pwa record --url https://example.com --desc "Extract data" --auth-profile auth-profiles/example.com/default/storageState.json

  # Re-run validation loop on an existing session
  pwa run --run runs/run-123

  # Re-run with more iterations
  pwa run --run runs/run-123 --max-iterations 10

  # Refine an existing script with manual feedback
  pwa refine --run runs/run-123 --feedback "Add pagination"
`);
}

function printBanner() {
  console.log(`
╔══════════════════════════════════════════════════╗
║           Playwright Automator v1.0              ║
║                                                  ║
║  Record browser actions → Generate automation    ║
║  scripts using Gemini AI                         ║
╚══════════════════════════════════════════════════╝
`);
}

async function runRefine(sessionDir: string, feedback: string, apiKey: string) {
  const sessionPath = join(sessionDir, 'session.json');
  if (!existsSync(sessionPath)) {
    console.error(`❌ Session not found: ${sessionPath}`);
    process.exit(1);
  }

  const session: RecordingSession = JSON.parse(readFileSync(sessionPath, 'utf-8'));
  const scriptPath = join(sessionDir, 'automation.ts');

  if (!existsSync(scriptPath)) {
    console.error(`❌ No script found to refine: ${scriptPath}`);
    process.exit(1);
  }

  const previousScript = readFileSync(scriptPath, 'utf-8');

  const result = await refineScript(session, previousScript, feedback, apiKey);

  // Save refined script (keep backup of previous)
  const backupPath = join(sessionDir, `automation.backup-${Date.now()}.ts`);
  writeFileSync(backupPath, previousScript, 'utf-8');
  writeFileSync(scriptPath, result.script, 'utf-8');

  console.log(`\n✅ Script refined!`);
  console.log(`   Script: ${scriptPath}`);
  console.log(`   Backup: ${backupPath}`);
  console.log(`   Strategy: ${result.strategy}`);
  if (result.apiEndpoints.length > 0) {
    console.log(`   API endpoints: ${result.apiEndpoints.join(', ')}`);
  }
}

async function main() {
  const args = parseArgs();

  // Default command
  if (!args.command) args.command = 'record';

  // Handle refinement mode
  if (args.command === 'refine') {
    const apiKey = args.apiKey || process.env.GEMINI_API_KEY;
    if (!apiKey) {
      console.error('❌ Gemini API key required. Use --key or set GEMINI_API_KEY env.');
      process.exit(1);
    }

    const runDir = args.run;
    if (!runDir) {
      console.error('❌ Run directory required. Use: pwa refine --run <dir> --feedback "..."');
      process.exit(1);
    }

    const feedback = args.feedback || 'Please improve the script';
    await runRefine(resolve(runDir), feedback, apiKey);
    return;
  }

  // run subcommand — re-run validation loop on an existing session
  if (args.command === 'run') {
    const apiKey = args.apiKey || process.env.GEMINI_API_KEY;
    if (!apiKey) {
      console.error('❌ Gemini API key required. Use --key or set GEMINI_API_KEY env.');
      process.exit(1);
    }

    const runDir = args.run;
    if (!runDir) {
      console.error('❌ Session directory required. Use: pwa run --run <dir>');
      process.exit(1);
    }

    const sessionDir = resolve(runDir);
    if (!existsSync(join(sessionDir, 'session.json'))) {
      console.error(`❌ session.json not found in ${sessionDir}`);
      process.exit(1);
    }
    if (!existsSync(join(sessionDir, 'automation.ts'))) {
      console.error(`❌ automation.ts not found in ${sessionDir}`);
      process.exit(1);
    }

    console.log('\n🔄 Running validation loop...\n');

    const loopResult = await runAgentLoop({
      sessionDir,
      apiKey,
      maxIterations: args.maxIterations,
      scriptTimeout: args.scriptTimeout,
    });

    if (loopResult.success) {
      console.log('\n─────────────────────────────────────────────────');
      console.log('✅ AUTOMATION COMPLETE');
      console.log('─────────────────────────────────────────────────');
      console.log(`   Iterations: ${loopResult.iterations}/${args.maxIterations || 5}`);
      console.log(`   Output:     ${join(sessionDir, 'output.json')} (${loopResult.outputItemCount} items)`);
      console.log(`   Script:     ${join(sessionDir, 'automation.ts')}`);
      console.log(`   History:    ${join(sessionDir, 'agent-loop-result.json')}`);
      console.log('─────────────────────────────────────────────────\n');
    } else {
      console.log('\n─────────────────────────────────────────────────');
      console.log('❌ VALIDATION FAILED');
      console.log('─────────────────────────────────────────────────');
      console.log(`   Iterations: ${loopResult.iterations}/${args.maxIterations || 5}`);
      console.log(`   History:    ${join(sessionDir, 'agent-loop-result.json')}`);
      console.log('');
      console.log('🔧 To retry or provide manual feedback:');
      console.log(`   pwa run --run ${runDir}`);
      console.log(`   pwa refine --run ${runDir} --feedback "your feedback"`);
      console.log('─────────────────────────────────────────────────\n');
      process.exit(1);
    }
    return;
  }

  // login subcommand
  if (args.command === 'login') {
    printBanner();
    let url = args.url;
    if (!url) {
      const rl = createReadlineInterface();
      url = await question(rl, '🌐 Enter the login URL: ');
      rl.close();
    }
    if (!url) {
      console.error('❌ URL is required');
      process.exit(1);
    }
    if (!url.startsWith('http://') && !url.startsWith('https://')) url = 'https://' + url;
    const domain = new URL(url).hostname;
    const profileName = args.profileName || 'default';
    await captureLoginStorageState({
      url,
      domain,
      profileName,
      headless: args.headless,
      notes: args.notes,
      cwd: process.cwd(),
    });
    return;
  }

  // `record` is the default behavior, so treat it as no-op.
  if (args.command === 'record') {
    args.command = undefined;
  }

  // Unknown subcommand → help.
  if (args.command) {
    console.error(`❌ Unknown command: ${args.command}`);
    printHelp();
    process.exit(1);
  }

  printBanner();

  let url = args.url;
  let description = args.description;
  let apiKey = args.apiKey || process.env.GEMINI_API_KEY;
  const outputDir = args.outputDir || DEFAULT_OUTPUT_DIR;

  // Interactive prompts for missing values
  if (!url || !description || (!apiKey && !args.skipGenerate)) {
    const rl = createReadlineInterface();

    if (!url) {
      url = await question(rl, '🌐 Enter the URL to automate: ');
    }
    if (!description) {
      description = await question(rl, '📝 Describe what you want to automate: ');
    }
    if (!apiKey && !args.skipGenerate) {
      apiKey = await question(rl, '🔑 Enter your Gemini API key (or set GEMINI_API_KEY env): ');
    }

    rl.close();
  }

  if (!url) {
    console.error('❌ URL is required');
    process.exit(1);
  }

  // Ensure URL has protocol
  if (!url.startsWith('http://') && !url.startsWith('https://')) {
    url = 'https://' + url;
  }

  if (!description) {
    console.error('❌ Description is required');
    process.exit(1);
  }

  // Ensure output directory exists
  mkdirSync(outputDir, { recursive: true });

  // Step 1: Record
  console.log('\n📹 Step 1: Recording browser session...\n');
  // If auth-profile not provided, try to auto-discover the newest auth profile for this domain
  let storageStatePath = args.authProfile;
  if (!storageStatePath) {
    const discovered = discoverAuthProfileForUrl(url, process.cwd());
    if (discovered) {
      storageStatePath = discovered.storageStatePath;
      console.log(`🔐 Using discovered auth profile: auth-profiles/${new URL(url).hostname}/${discovered.profileName}/storageState.json`);
    }
  }

  const session = await startRecording({
    url,
    description,
    outputDir,
    headless: args.headless,
    storageStatePath,
  });

  if (args.skipGenerate) {
    console.log('✅ Recording complete (script generation skipped).');
    console.log(`   Session dir: ${join(outputDir, session.id)}\n`);
    return;
  }

  if (!apiKey) {
    console.log('⚠️  No API key provided. Skipping script generation.');
    console.log(`   Session dir: ${join(outputDir, session.id)}\n`);
    return;
  }

  // Step 2: Generate script
  console.log('\n🤖 Step 2: Generating Playwright automation script...\n');

  try {
    const result = await generateScript(session, apiKey);

    const sessionDir = join(outputDir, session.id);
    const scriptPath = join(sessionDir, 'automation.ts');

    // Save the generated script
    writeFileSync(scriptPath, result.script, 'utf-8');

    // Save generation metadata
    writeFileSync(
      join(sessionDir, 'generation-info.json'),
      JSON.stringify({
        strategy: result.strategy,
        apiEndpoints: result.apiEndpoints,
        explanation: result.explanation,
        generatedAt: new Date().toISOString(),
        promptVersion: result.promptVersion,
        irHash: result.irHash,
        templatesHash: result.templatesHash,
      }, null, 2),
      'utf-8',
    );

    // Save execution plan if available
    if (result.executionPlan) {
      writeFileSync(
        join(sessionDir, 'execution-plan.json'),
        JSON.stringify(result.executionPlan, null, 2),
        'utf-8',
      );
      // Attach to session for use during refinement
      session.executionPlan = result.executionPlan;
    }

    // Create a convenience run script
    const runScript = `#!/bin/bash
# Run the generated Playwright automation script
# Make sure you have playwright installed: npm i playwright
cd "$(dirname "$0")"
npx tsx automation.ts
`;
    writeFileSync(join(sessionDir, 'run.sh'), runScript, { mode: 0o755 });

    console.log(`   Strategy: ${result.strategy}`);
    if (result.apiEndpoints.length > 0) {
      console.log(`   API endpoints: ${result.apiEndpoints.join(', ')}`);
    }

    // Step 3: Validate & refine loop
    const maxIter = args.maxIterations || 5;
    console.log(`\n🔄 Step 3: Validating script...\n`);

    const loopResult = await runAgentLoop({
      sessionDir,
      apiKey,
      maxIterations: maxIter,
      scriptTimeout: args.scriptTimeout,
    });

    if (loopResult.success) {
      console.log('\n─────────────────────────────────────────────────');
      console.log('✅ AUTOMATION COMPLETE');
      console.log('─────────────────────────────────────────────────');
      console.log(`   Iterations: ${loopResult.iterations}/${maxIter}`);
      console.log(`   Strategy:   ${result.strategy}`);
      console.log(`   Output:     ${join(sessionDir, 'output.json')} (${loopResult.outputItemCount} items)`);
      console.log(`   Script:     ${scriptPath}`);
      console.log(`   History:    ${join(sessionDir, 'agent-loop-result.json')}`);
      console.log('');
      console.log('🔧 To re-run or further refine:');
      console.log(`   pwa run --run ${sessionDir}`);
      console.log(`   pwa refine --run ${sessionDir} --feedback "your feedback"`);
      console.log('─────────────────────────────────────────────────\n');
    } else {
      console.log('\n─────────────────────────────────────────────────');
      console.log('❌ VALIDATION FAILED after ' + loopResult.iterations + ' iterations');
      console.log('─────────────────────────────────────────────────');
      console.log(`   Script:     ${scriptPath}`);
      console.log(`   History:    ${join(sessionDir, 'agent-loop-result.json')}`);
      console.log('');
      console.log('🔧 To retry or provide manual feedback:');
      console.log(`   pwa run --run ${sessionDir}`);
      console.log(`   pwa refine --run ${sessionDir} --feedback "your feedback"`);
      console.log('─────────────────────────────────────────────────\n');
    }

  } catch (error: any) {
    console.error(`\n❌ Script generation failed: ${error.message}`);
    console.log('   The recording data is still saved. You can retry with:');
    console.log(`   Check the session dir: ${join(outputDir, session.id)}\n`);
    process.exit(1);
  }
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
