# Investigation: When was OpenRouter support added to openai/codex?

## Task
Clone github.com/openai/codex and investigate git history to determine when openrouter support was first added.

## Investigation Log


### 2025-11-09 - Initial Investigation

**Repository Cloned:** github.com/openai/codex

**Search Method:** Used git log with -S flag to search for "openrouter" in code changes

**Finding:** Earliest commit mentioning openrouter:
- Commit: eafbc75612ead742fdd8bb19ff8d531e2bb2df77
- Author: Daniel Nakov <daniel.nakov@gmail.com>
- Date: Sun Apr 20 23:59:34 2025 -0400
- Message: "feat: support multiple providers via Responses-Completion transformation (#247)"
- PR: #247

**What was added:**
OpenRouter was added as one of the supported providers in a multi-provider support feature. The commit added:
- openrouter provider configuration with:
  - name: "OpenRouter"
  - baseURL: "https://openrouter.ai/api/v1"
  - envKey: "OPENROUTER_API_KEY"

This was part of a larger change that added support for multiple providers (openai, openrouter, gemini, etc.) via a transformation between Responses API and Completion API.

**Files changed:** 11 files, 1870 insertions(+), 83 deletions(-)
Main new files:
- codex-cli/src/utils/providers.ts
- codex-cli/src/utils/responses.ts
- codex-cli/tests/responses-chat-completions.test.ts


### Timeline of OpenRouter-related commits

**First mention:** April 20, 2025
- Commit: eafbc756
- Date: 2025-04-20
- Message: "feat: support multiple providers via Responses-Completion transformation (#247)"
- This is the FIRST commit that added OpenRouter support

**Follow-up commits:**
1. April 21, 2025 (eafbc756) - Initial provider support added
2. April 21, 2025 (e7a3eec9) - Documentation for non-OpenAI providers 
3. April 25, 2025 (1ef8e8af) - Provider config documentation
4. May 7, 2025 (86022f09) - Read model_provider from config.toml (#853)
5. June 27, 2025 (a331a67b) - Changed built_in_model_providers
6. August 8, 2025 (408c7ca1) - Removed TypeScript code (migration to Rust)

**Key observation:**
OpenRouter support was part of the initial multi-provider support feature that also added:
- Gemini
- Ollama
- Mistral
- DeepSeek
- xAI
- Groq

All these providers were added simultaneously in the same commit on April 20, 2025.


### Final Analysis

**Answer to the question:** OpenRouter support was first added on April 20, 2025.

**Context:**
- It was not added in isolation, but as part of a larger multi-provider support initiative
- The implementation was well-architected using a transformation layer approach
- The feature has been maintained through significant codebase changes (TypeScript → Rust migration)
- Co-authors on the PR included OpenAI team members (Thibault Sottiaux, Fouad Matin)

**Files saved:**
- openrouter-initial-commit.diff - Full diff of the initial commit that added OpenRouter support
- notes.md - This investigation log
- README.md - Final report with findings

**Repository cleanup:**
- Removing the cloned codex repository to keep only investigation artifacts
- Following AGENTS.md guidelines: no full copies of fetched code in final commit

