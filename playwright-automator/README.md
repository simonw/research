# Playwright Automator

Record browser actions and generate Playwright automation scripts using Gemini AI.

## How It Works

1. **Setup** — Provide a Gemini API key and a target URL
2. **Record** — A Playwright browser opens; you perform your task manually (e.g., log in, navigate to DMs)
3. **Capture** — All network traffic (HAR), user actions (clicks, typing), cookies, and auth headers are recorded
4. **Generate** — Gemini AI analyzes the captured data and generates a standalone Playwright script
5. **Run** — Execute the generated script to automate what you did manually

The tool prioritizes **API interception over DOM scraping**. It analyzes the HAR file to find the best API endpoints and generates scripts that use `page.waitForResponse()` or `page.route()` to intercept API data directly.

## Quick Start

```bash
# Install dependencies
npm install
npx playwright install chromium

# Build the CLI (creates ./dist)
npm run build

# (Dev) Install as a global CLI on your machine
npm link

# Run interactively
playwright-automator

# Record (explicit)
playwright-automator record --url https://example.com --desc "Get all messages"

# Capture reusable login/auth state (headed; supports 2FA)
playwright-automator login --url https://example.com/login --profile default

# Record using an auth profile (loads storageState.json)
playwright-automator record --url https://example.com --desc "Extract data" --auth-profile auth-profiles/example.com/default/storageState.json
```

## Installing as a CLI

This repo is set up as a proper Node CLI with a `bin` entry (`playwright-automator → dist/index.js`).

```bash
# Option A: install globally from the repo (will run the build via npm "prepare")
cd playwright-automator
npm install
npm install -g .

# Option B: dev workflow (symlink)
npm install
npm run build
npm link
```

## Usage

### Interactive Mode

```
$ playwright-automator

╔══════════════════════════════════════════════════╗
║           Playwright Automator v1.0              ║
╚══════════════════════════════════════════════════╝

🌐 Enter the URL to automate: instagram.com
📝 Describe what you want to automate: Get all my DMs from instagram
🔑 Enter your Gemini API key: <your-key>

📹 Step 1: Recording browser session...
🌐 Opening browser to: https://instagram.com

─────────────────────────────────────────────────
🎬 RECORDING IN PROGRESS
Interact with the browser to perform your task.
Press ENTER to stop recording.
─────────────────────────────────────────────────

⏹️  Stopping recording...
✅ Browser closed. Processing recording data...

🤖 Step 2: Generating Playwright automation script...

✅ AUTOMATION SCRIPT GENERATED!
   Strategy: API Interception
   API endpoints targeted:
     - /api/v1/direct_v2/inbox
```

### Command-Line Mode

```bash
# Full automation
playwright-automator record --url https://example.com --desc "Scrape all articles" --key $GEMINI_API_KEY

# Record only (no script generation)
playwright-automator record --url https://example.com --desc "Explore the site" --skip-generate

# Refine an existing script
playwright-automator refine --run runs/run-123 --feedback "Add pagination for all pages"
```

### Environment Variables

```bash
export GEMINI_API_KEY=your_api_key_here
```

## Output Structure

Each run creates a folder in `./runs/`:

```
runs/run-1234567890-abc12345/
├── recording.har         # Complete HAR file (all network traffic)
├── session.json          # Full session metadata
├── actions.json          # Recorded user actions (clicks, typing, navigation)
├── auth.json             # Extracted auth data (cookies, headers, auth method)
├── storageState.json     # Playwright storage state for auth/session replay (new)
├── ir.json               # Deterministic endpoint catalog extracted from HAR (new)
├── automation.ts         # Generated Playwright script
├── generation-info.json  # Script generation metadata
├── run.sh                # Convenience script to run automation
└── screenshots/          # Captured screenshots
    ├── 0-initial.png
    └── 1-final.png
```

### Running the Generated Script

```bash
cd runs/run-1234567890-abc12345/
npx tsx automation.ts
# Output data is saved to output.json
```

## Architecture

```
┌─────────────────┐     ┌──────────────┐     ┌─────────────┐
│   CLI / Input    │────▶│   Recorder   │────▶│ HAR Analyzer │
│ (url, desc, key) │     │ (Playwright) │     │  (Parser)    │
└─────────────────┘     └──────────────┘     └──────┬──────┘
                                                     │
                        ┌──────────────┐             │
                        │   Gemini AI  │◀────────────┘
                        │  (Generator) │
                        └──────┬───────┘
                               │
                        ┌──────▼───────┐
                        │  automation  │
                        │    .ts       │
                        └──────────────┘
```

### Components

- **`src/index.ts`** — CLI entry point with interactive prompts and argument parsing
- **`src/recorder.ts`** — Launches Playwright browser with HAR recording, action tracking, and screenshot capture
- **`src/har-analyzer.ts`** — Parses HAR files, filters noise (analytics, static assets), extracts API endpoints and auth
- **`src/gemini.ts`** — Sends recorded data to Gemini AI to generate Playwright automation scripts
- **`src/types.ts`** — TypeScript type definitions
- **`src/test-server.ts`** — Sample web app for testing (login, dashboard, messages)

### Key Design Decisions

1. **API-first approach** — Generated scripts intercept API responses via `page.waitForResponse()` rather than scraping the DOM. API data is more structured and reliable.

2. **HAR as single source of truth** — Playwright's `recordHar` captures complete request/response data including headers, cookies, bodies, and timing. This gives the LLM maximum context.

3. **Per-run isolation** — Each recording creates a self-contained folder with everything needed to reproduce and refine the automation.

4. **Refinement loop** — Use `--refine` to iteratively improve scripts. The session data, HAR, and previous script are all available for the LLM to reference.

## Testing

```bash
# Run the test server
npx tsx src/test-server.ts

# Run the E2E test suite
npx tsx src/e2e-test-headless.ts

# Run with Gemini script generation
GEMINI_API_KEY=your_key npx tsx src/e2e-test-headless.ts
```

The E2E test validates the full pipeline: server startup, browser recording, login flow, HAR capture, API extraction, response body parsing, auth detection, and session data persistence.

## Reference

Built using patterns from [unbrowse-openclaw](https://github.com/lekt9/unbrowse-openclaw) for HAR parsing and API endpoint extraction. The Gemini integration and script generation pipeline are original.
