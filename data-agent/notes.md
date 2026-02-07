# data-agent — Development Notes

## Approach

Built a CLI tool that autonomously explores websites, discovers API endpoints, and generates standalone Playwright scripts to extract structured data.

### Architecture Decisions

1. **Copied snapshot.ts from agent-browser** — MIT-licensed compact accessibility snapshot with @refs for element targeting. This gives the LLM a token-efficient view of the page (~200-400 tokens vs thousands for raw DOM).

2. **Adapted patterns from Skyvern** — The explore loop follows the plan→act→observe→record→evaluate pattern. Each action carries intent metadata (reasoning, intention, confidence) for better analysis.

3. **Rewritten analysis pipeline from playwright-automator** — Clean TypeScript rewrite of HAR analysis, IR building, workflow detection, action-API correlation, response schemas, and request templates. Same scoring heuristics (JSON +50, API-like +20, HTML -80, etc.) but cleaner API surfaces.

4. **Pluggable LLM providers** — Supports both Gemini and Claude via a clean `LlmProvider` interface. Default Gemini for script generation (good at code), Claude available as alternative.

5. **MCP server** — Exposes `explore`, `replay`, and `list` tools so any coding agent can use data-agent as a capability.

### Key Files

- `src/browser/snapshot.ts` — Copied from vercel-labs/agent-browser, adapted imports
- `src/explore/explorer.ts` — AI agent loop (Skyvern-style)
- `src/analyze/har-analyzer.ts` — Full analysis pipeline orchestrator
- `src/analyze/ir-builder.ts` — Endpoint scoring and IR construction
- `src/analyze/workflow-analyzer.ts` — List→detail, pagination, variable flow detection
- `src/validate/validator.ts` — Script execution + self-correcting refinement loop
- `src/mcp/server.ts` — MCP tool server
- `prompts/explore.md` — System prompt for the explore agent
- `prompts/generate.md` — System prompt for Playwright script generation
- `prompts/diagnose.md` — System prompt for error diagnosis and script refinement

### What Worked

- TypeScript compilation clean on first build (0 errors)
- All 20 source files compile and produce working dist
- CLI entry point with proper parseArgs handling
- Browser detection finds system Chrome on macOS
- Registry system works for persistence

### Reference Code Studied

- `playwright-automator/src/har-analyzer.ts` — HAR parsing, domain filtering, auth detection
- `playwright-automator/src/ir-builder.ts` — Path normalization, endpoint scoring
- `playwright-automator/src/workflow-analyzer.ts` — Pattern detection algorithms
- `playwright-automator/src/action-api-correlator.ts` — Time-window correlation
- `playwright-automator/src/response-schema.ts` — Schema extraction
- `playwright-automator/src/request-templates.ts` — Template sanitization
- `playwright-automator/src/runner.ts` — Script execution and error classification
- `playwright-automator/prompts/v1/*.md` — Prompt engineering patterns
- `agent-browser/src/snapshot.ts` — Accessibility snapshot with refs
