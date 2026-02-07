#!/usr/bin/env node

/**
 * data-agent — AI-powered data extractor with explore/replay.
 *
 * Usage:
 *   npx data-agent "get a list of 10 chatgpt conversations"  # Auto mode
 *   npx data-agent replay chatgpt.com                         # Replay saved script
 *   npx data-agent list                                       # List saved extractors
 *   npx data-agent login https://chatgpt.com                  # Login and save auth
 *   npx data-agent mcp                                        # Start MCP server
 */

import 'dotenv/config';

import { parseArgs } from 'node:util';
import { resolve } from 'node:path';
import { existsSync, mkdirSync } from 'node:fs';
import { getChromePath } from './browser/detect.js';
import { parseIntent } from './explore/intent-parser.js';
import { explore } from './explore/explorer.js';
import { analyze } from './analyze/har-analyzer.js';
import { generateScript } from './generate/script-generator.js';
import { validate } from './validate/validator.js';
import { replay } from './replay/replay.js';
import { listRegistry } from './replay/registry.js';
import { login } from './auth/login.js';
import { createLlmProvider } from './llm/provider.js';

const SESSIONS_DIR = resolve(process.cwd(), 'sessions');

function ensureSessionsDir(): void {
  if (!existsSync(SESSIONS_DIR)) {
    mkdirSync(SESSIONS_DIR, { recursive: true });
  }
}

function printUsage(): void {
  console.log(`
data-agent — AI-powered data extractor with explore/replay

Usage:
  data-agent "<description>"              Extract data (auto mode)
  data-agent replay <domain>              Replay saved extractor
  data-agent list                         List saved extractors
  data-agent login <url>                  Login and save auth state
  data-agent mcp                          Start MCP server

Options:
  --provider <gemini|claude>    LLM provider (default: gemini)
  --headless                    Run browser headlessly
  --help                        Show this help

Environment:
  GEMINI_API_KEY                Gemini API key
  ANTHROPIC_API_KEY             Claude API key
  CHROME_PATH                   Custom Chrome path
`);
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);

  if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
    printUsage();
    process.exit(0);
  }

  // Parse known flags
  const { values, positionals } = parseArgs({
    args,
    options: {
      provider: { type: 'string', default: 'gemini' },
      headless: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false },
    },
    allowPositionals: true,
  });

  if (values.help) {
    printUsage();
    process.exit(0);
  }

  const command = positionals[0];

  if (!command) {
    printUsage();
    process.exit(1);
  }

  // Check for system Chrome
  const chromePath = getChromePath();
  if (chromePath) {
    console.log(`Browser: ${chromePath}`);
  } else {
    console.log('Browser: Playwright default (no system Chrome found)');
  }

  // Route commands
  switch (command) {
    case 'replay': {
      const domain = positionals[1];
      if (!domain) {
        console.error('Usage: data-agent replay <domain>');
        process.exit(1);
      }
      await replay(domain);
      break;
    }

    case 'list': {
      await listRegistry();
      break;
    }

    case 'login': {
      const url = positionals[1];
      if (!url) {
        console.error('Usage: data-agent login <url>');
        process.exit(1);
      }
      await login(url, { chromePath, headless: false });
      break;
    }

    case 'mcp': {
      const { startMcpServer } = await import('./mcp/server.js');
      await startMcpServer();
      break;
    }

    default: {
      // Auto mode: treat the positional as a natural language description
      const description = positionals.join(' ');
      await runAutoMode(description, {
        provider: values.provider as string,
        headless: values.headless as boolean,
        chromePath,
      });
    }
  }
}

interface AutoModeOptions {
  provider: string;
  headless: boolean;
  chromePath?: string;
}

async function runAutoMode(description: string, options: AutoModeOptions): Promise<void> {
  ensureSessionsDir();

  console.log(`\nTask: "${description}"`);
  console.log('Mode: Auto (explore -> analyze -> generate -> validate -> publish)\n');

  // 1. Parse intent
  console.log('Step 1: Parsing intent...');
  const llm = createLlmProvider(options.provider);
  const intent = await parseIntent(description, llm);
  console.log(`  Domain: ${intent.domain}`);
  console.log(`  URL: ${intent.url}`);
  console.log(`  Task: ${intent.task}`);
  console.log(`  Auth required: ${intent.requiresAuth}`);

  // 2. Check registry for existing extractor
  // (TODO: registry lookup — for now, always explore)

  // 3. Auth check
  let authStatePath: string | undefined;
  if (intent.requiresAuth) {
    const { discoverAuthProfile } = await import('./auth/profiles.js');
    const profile = discoverAuthProfile(intent.domain);
    if (profile) {
      console.log(`\nUsing saved auth profile: ${profile.profileName}`);
      authStatePath = profile.storageStatePath;
    } else {
      console.log('\nAuth required — launching browser for login...');
      authStatePath = await login(intent.url, {
        chromePath: options.chromePath,
        headless: false,
      });
    }
  }

  // 4. Explore
  console.log('\nStep 2: Exploring site...');
  const exploreResult = await explore({
    task: intent.task,
    url: intent.url,
    llm,
    chromePath: options.chromePath,
    headless: options.headless,
    authStatePath,
    sessionsDir: SESSIONS_DIR,
  });
  console.log(`  Actions: ${exploreResult.actions.length}`);
  console.log(`  APIs seen: ${exploreResult.apisSeen.length}`);

  // 5. Analyze
  console.log('\nStep 3: Analyzing captured traffic...');
  const analysis = await analyze(exploreResult);
  console.log(`  Endpoints: ${analysis.ir.endpoints.length}`);
  console.log(`  Workflow patterns: ${analysis.workflow.patterns.length}`);

  // 6. Generate script
  console.log('\nStep 4: Generating automation script...');
  const generation = await generateScript({
    analysis,
    task: intent.task,
    url: intent.url,
    llm,
    sessionDir: exploreResult.sessionDir,
  });
  console.log(`  Strategy: ${generation.strategy}`);

  // 7. Validate (execute, verify, retry)
  console.log('\nStep 5: Validating script...');
  const validationResult = await validate({
    sessionDir: exploreResult.sessionDir,
    llm,
    maxIterations: 5,
    scriptTimeout: 120_000,
  });

  if (validationResult.success) {
    console.log(`\nSuccess! Output written to ${exploreResult.sessionDir}/output.json`);
    console.log(`  Items: ${validationResult.outputItemCount}`);
    console.log(`  Iterations: ${validationResult.iterations}`);

    // 8. Publish to registry
    console.log('\nStep 6: Publishing to registry...');
    const { publishToRegistry } = await import('./replay/registry.js');
    await publishToRegistry({
      domain: intent.domain,
      task: intent.task,
      sessionDir: exploreResult.sessionDir,
    });
    console.log('  Published for future replay.');
  } else {
    console.error(`\nFailed after ${validationResult.iterations} iterations.`);
    console.error(`  Last exit code: ${validationResult.finalExitCode}`);
    process.exit(1);
  }
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
