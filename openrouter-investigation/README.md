# OpenRouter Support in openai/codex - Investigation Report

## Summary

OpenRouter support was first added to the openai/codex repository on **April 20, 2025** as part of a comprehensive multi-provider support feature.

## Key Findings

### Initial Implementation

**Commit:** `eafbc75612ead742fdd8bb19ff8d531e2bb2df77`  
**Date:** Sunday, April 20, 2025 at 23:59:34 -0400  
**Author:** Daniel Nakov <daniel.nakov@gmail.com>  
**PR:** #247  
**Title:** "feat: support multiple providers via Responses-Completion transformation"

### What Was Added

OpenRouter was added as one of several alternative model providers alongside:
- OpenAI (default)
- Gemini
- Ollama
- Mistral
- DeepSeek
- xAI
- Groq

The implementation introduced a provider configuration system with the following settings for OpenRouter:

```typescript
openrouter: {
  name: "OpenRouter",
  baseURL: "https://openrouter.ai/api/v1",
  envKey: "OPENROUTER_API_KEY",
}
```

### Implementation Details

The initial commit made substantial changes:
- **11 files changed**
- **1,870 insertions, 83 deletions**

Key new files created:
- `codex-cli/src/utils/providers.ts` - Provider configuration
- `codex-cli/src/utils/responses.ts` - API transformation layer
- `codex-cli/tests/responses-chat-completions.test.ts` - Test suite

The approach implemented a transformation layer between the Responses API and Completion API, allowing support for multiple providers while minimizing changes to the core codebase.

### Evolution Timeline

1. **April 20, 2025** - Initial multi-provider support added (eafbc756)
2. **April 21, 2025** - Documentation for non-OpenAI providers (e7a3eec9)
3. **April 25, 2025** - Provider config documentation (#653)
4. **May 7, 2025** - Added config.toml support for model_provider (#853)
5. **June 27, 2025** - Changed built_in_model_providers configuration (#1407)
6. **August 8, 2025** - Removed TypeScript code, migrated to Rust (#2048)

### Current Status

As of the latest repository state, OpenRouter support is:
- ✅ Fully supported and documented
- ✅ Available via `--provider openrouter` CLI flag
- ✅ Configurable in config.toml
- ✅ Requires `OPENROUTER_API_KEY` environment variable

The feature was initially implemented in TypeScript and later migrated to Rust as part of the CLI rewrite in August 2025.

## Usage

To use OpenRouter with codex:

```bash
# Set the API key
export OPENROUTER_API_KEY="your-openrouter-key-here"

# Use with CLI flag
codex --provider openrouter

# Or configure in config.toml
[model_providers.openrouter]
name = "OpenRouter"
baseURL = "https://openrouter.ai/api/v1"
envKey = "OPENROUTER_API_KEY"
```

## Conclusion

OpenRouter support was added as part of the initial multi-provider architecture on April 20, 2025, enabling codex users to leverage OpenRouter's model marketplace alongside other AI providers. The feature has been maintained through the TypeScript-to-Rust migration and remains a core supported provider.

