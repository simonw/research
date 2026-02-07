## Stage 1: Project setup + browser + snapshots
**Goal**: New project with system Chrome and compact snapshots working.
**Success Criteria**: TypeScript compiles, CLI shows help, browser detection works.
**Tests**: `npx tsx src/index.ts --help` prints usage, `list` command works.
**Status**: Complete

## Stage 2: Explorer agent loop
**Goal**: AI-driven exploration that discovers data patterns autonomously.
**Success Criteria**: Explorer can navigate, take snapshots, call LLM for decisions.
**Tests**: Explorer module compiles, prompt loads, action types work.
**Status**: Complete

## Stage 3: Analysis pipeline
**Goal**: Turn recorded HAR + actions into structured analysis.
**Success Criteria**: HAR parsing, IR building, workflow detection, correlation all work.
**Tests**: All analyze/* modules compile and export correct functions.
**Status**: Complete

## Stage 4: Script generation + validation
**Goal**: LLM generates deterministic Playwright scripts from analysis.
**Success Criteria**: Script generator builds prompts, validator runs scripts and classifies errors.
**Tests**: Generate and validate modules compile with all prompt files.
**Status**: Complete

## Stage 5: Registry + replay + auth
**Goal**: Publish scripts and replay without LLM.
**Success Criteria**: Registry persists at ~/.data-agent, replay runs saved scripts.
**Tests**: `data-agent list` shows empty registry, auth saves storageState.
**Status**: Complete

## Stage 6: MCP server + CLI polish
**Goal**: Agent integration and ship v1.
**Success Criteria**: MCP server exposes explore/replay/list tools, CLI has proper help.
**Tests**: CLI --help works, MCP server module compiles.
**Status**: Complete
