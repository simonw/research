#!/usr/bin/env node
/**
 * Playwright Automator — Record browser actions and generate automation scripts.
 *
 * Usage:
 *   npx tsx src/index.ts                    # Interactive mode
 *   npx tsx src/index.ts --url <url> --desc <description> --key <gemini-api-key>
 *   npx tsx src/index.ts --refine <session-dir> --feedback "fix the pagination"
 */

import { mkdirSync, existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { startRecording } from './recorder.js';
import { generateScript, refineScript } from './gemini.js';
import { captureLoginStorageState } from './login.js';
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
  refine?: string;
  feedback?: string;
  headless?: boolean;
  skipGenerate?: boolean;
  authProfile?: string;
  profileName?: string;
  notes?: string;
} {
  const args = process.argv.slice(2);
  const result: Record<string, string | boolean> = {};

  // subcommand support (backwards compatible):
  // `npx tsx src/index.ts login --url ...`
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
        result.refine = args[++i];
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
  npx tsx src/index.ts [command] [options]

COMMANDS:
  (default)            Record + generate (current behavior)
  login                Capture reusable auth storageState (headed; supports 2FA)

OPTIONS:
  --url, -u <url>          Target URL to automate
  --desc, -d <description> What you want to automate
  --key, -k <key>          Gemini API key (or set GEMINI_API_KEY env)
  --output, -o <dir>       Output directory (default: ./runs)
  --headless               Run browser in headless mode
  --skip-generate          Only record, don't generate script
  --auth-profile <path>    Load a Playwright storageState.json before recording (auth replay)
  --refine, -r <dir>       Refine script in existing session dir
  --profile <name>         (login cmd) profile name (default: default)
  --notes <text>           (login cmd) notes for this auth profile
  --feedback, -f <text>    Feedback for refinement
  --help, -h               Show this help

WORKFLOW:
  1. Run the tool and provide a URL + task description
  2. A browser opens — perform the task manually
  3. Press ENTER when done to stop recording
  4. The tool captures HAR traffic, actions, and cookies
  5. Gemini AI generates a Playwright script
  6. The script is saved in a per-run folder

EXAMPLES:
  # Interactive mode
  npx tsx src/index.ts

  # Command-line mode
  npx tsx src/index.ts --url https://example.com --desc "Scrape all articles"

  # Capture login state (for sites with 2FA)
  npx tsx src/index.ts login --url https://example.com/login --profile default

  # Record using an existing auth profile (storageState.json)
  npx tsx src/index.ts --url https://example.com --desc "Extract data" --auth-profile auth-profiles/example.com/default/storageState.json

  # Refine an existing script
  npx tsx src/index.ts --refine runs/run-123 --feedback "Add pagination"
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

  // Handle refinement mode
  if (args.refine) {
    const apiKey = args.apiKey || process.env.GEMINI_API_KEY;
    if (!apiKey) {
      console.error('❌ Gemini API key required. Use --key or set GEMINI_API_KEY env.');
      process.exit(1);
    }
    const feedback = args.feedback || 'Please improve the script';
    await runRefine(resolve(args.refine), feedback, apiKey);
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
  const session = await startRecording({
    url,
    description,
    outputDir,
    headless: args.headless,
    storageStatePath: args.authProfile,
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

    // Create a convenience run script
    const runScript = `#!/bin/bash
# Run the generated Playwright automation script
# Make sure you have playwright installed: npm i playwright
cd "$(dirname "$0")"
npx tsx automation.ts
`;
    writeFileSync(join(sessionDir, 'run.sh'), runScript, { mode: 0o755 });

    console.log('─────────────────────────────────────────────────');
    console.log('✅ AUTOMATION SCRIPT GENERATED!');
    console.log('─────────────────────────────────────────────────');
    console.log(`   Strategy: ${result.strategy}`);
    if (result.apiEndpoints.length > 0) {
      console.log(`   API endpoints targeted:`);
      for (const ep of result.apiEndpoints) {
        console.log(`     - ${ep}`);
      }
    }
    console.log('');
    console.log('📁 Output files:');
    console.log(`   Session dir:  ${sessionDir}`);
    console.log(`   Script:       ${scriptPath}`);
    console.log(`   HAR file:     ${session.harFilePath}`);
    console.log(`   Actions:      ${join(sessionDir, 'actions.json')}`);
    console.log(`   Auth data:    ${join(sessionDir, 'auth.json')}`);
    console.log('');
    console.log('🚀 To run the automation:');
    console.log(`   cd ${sessionDir}`);
    console.log(`   npx tsx automation.ts`);
    console.log('');
    console.log('🔧 To refine the script:');
    console.log(`   npx tsx src/index.ts --refine ${sessionDir} --feedback "your feedback"`);
    console.log('─────────────────────────────────────────────────\n');

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
