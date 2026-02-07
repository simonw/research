# data-agent

AI-powered data extractor with explore/replay. Extract structured data from any website using natural language.

```bash
npx data-agent "get the top 10 stories from hacker news"
```

## How It Works

1. **Parse Intent** — Natural language → domain + task + auth requirements
2. **Explore** — AI agent navigates the site, discovers API endpoints via HAR recording
3. **Analyze** — HAR → IR → workflow patterns → action-API correlation
4. **Generate** — LLM writes a standalone Playwright script from the analysis
5. **Validate** — Execute script, verify output, auto-fix errors (up to 5 iterations)
6. **Publish** — Save to registry for future deterministic replay

## Modes

| Mode | Command | LLM needed? | Description |
|------|---------|-------------|-------------|
| **Auto** | `data-agent "description"` | Yes | Full: explore → analyze → generate → validate → publish |
| **Replay** | `data-agent replay <domain>` | No | Run saved script deterministically |
| **MCP** | `data-agent mcp` | Agent's LLM | MCP server for any coding agent |

## Usage

```bash
# Auto mode — explore and extract
data-agent "get a list of my chatgpt conversations"
data-agent "get the top stories from hacker news"

# Replay — run saved extractor (no LLM needed)
data-agent replay news.ycombinator.com
data-agent replay chatgpt.com

# List saved extractors
data-agent list

# Login to a site (saves auth for future use)
data-agent login https://chatgpt.com

# Start MCP server
data-agent mcp
```

## Options

```
--provider <gemini|claude>    LLM provider (default: gemini)
--headless                    Run browser headlessly
--help                        Show help
```

## Environment Variables

```
GEMINI_API_KEY                Gemini API key (required for gemini provider)
ANTHROPIC_API_KEY             Claude API key (required for claude provider)
CHROME_PATH                   Custom Chrome executable path
```

## Architecture

```
data-agent/
├── src/
│   ├── index.ts                 # CLI entry point
│   ├── types.ts                 # Shared types
│   ├── browser/
│   │   ├── detect.ts            # System Chrome detection
│   │   └── snapshot.ts          # Accessibility snapshots (from agent-browser)
│   ├── explore/
│   │   ├── explorer.ts          # AI agent loop
│   │   └── intent-parser.ts     # NL → domain + task
│   ├── analyze/
│   │   ├── har-analyzer.ts      # HAR parsing + pipeline orchestrator
│   │   ├── ir-builder.ts        # Endpoint scoring and ranking
│   │   ├── workflow-analyzer.ts # Pattern detection
│   │   ├── correlator.ts        # Action→API correlation
│   │   ├── response-schema.ts   # JSON schema extraction
│   │   └── templates.ts         # Request template sanitization
│   ├── generate/
│   │   └── script-generator.ts  # LLM generates automation.ts
│   ├── validate/
│   │   └── validator.ts         # Execute + validate + auto-fix
│   ├── replay/
│   │   ├── replay.ts            # Deterministic replay
│   │   └── registry.ts          # Global extractor registry
│   ├── auth/
│   │   ├── login.ts             # Browser-based login
│   │   └── profiles.ts          # Auth profile management
│   ├── llm/
│   │   └── provider.ts          # Pluggable LLM (Gemini, Claude)
│   └── mcp/
│       └── server.ts            # MCP tool server
└── prompts/
    ├── explore.md               # Explore agent prompt
    ├── generate.md              # Script generation prompt
    └── diagnose.md              # Error diagnosis prompt
```

## Generated Scripts

Generated `automation.ts` files are standalone Playwright — zero runtime dependency on data-agent:

```typescript
import { chromium } from 'playwright';
import { writeFileSync } from 'fs';

async function main() {
  const browser = await chromium.launch({
    executablePath: process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    headless: true,
  });
  const context = await browser.newContext({ storageState: './storageState.json' });
  const page = await context.newPage();

  const listPromise = page.waitForResponse(
    r => r.url().includes('/backend-api/conversations') && r.status() === 200
  );
  await page.goto('https://chatgpt.com');
  const data = await (await listPromise).json();

  writeFileSync('./output.json', JSON.stringify(data, null, 2));
  await browser.close();
}

main();
```

## Key Patterns

### Explore Phase (AI Agent Loop)

Uses compact accessibility snapshots with @refs for token-efficient page representation. The LLM decides: click @e3, type into @e5, scroll down, navigate to URL, or declare "done" when enough API patterns are observed.

### Analysis Pipeline

- **HAR Parsing**: Filter static assets, analytics, tracking domains
- **Endpoint Scoring**: JSON +50, API-like +20, mutating +10, HTML -80, telemetry -30
- **Workflow Detection**: List→detail pairs, pagination (offset/cursor), variable flow
- **Action-API Correlation**: Link user actions to API calls within 2-second window
- **Schema Extraction**: Replace truncation with structural type information

### Script Generation

- API-first: `waitForResponse()` with predicate form
- Listener-before-action: Avoid race conditions
- UI-driven interception: Click items to trigger APIs (not direct fetch)
- Auth via storageState: `browser.newContext({ storageState })` pattern

### Self-Correcting Validation

Execute → classify errors (403/401/429/timeout/selector) → generate targeted feedback → refine script → retry (up to 5 iterations).

## MCP Integration

Connect from any coding agent:

```json
{
  "mcpServers": {
    "data-agent": {
      "command": "npx",
      "args": ["tsx", "/path/to/data-agent/src/index.ts", "mcp"]
    }
  }
}
```

Available tools: `explore`, `replay`, `list`.

## Credits

- **Accessibility snapshots**: Adapted from [vercel-labs/agent-browser](https://github.com/vercel-labs/agent-browser) (MIT)
- **Analysis patterns**: Rewritten from [playwright-automator](../playwright-automator/)
- **Agent loop patterns**: Inspired by [Skyvern](https://github.com/Skyvern-AI/skyvern)
