Anthropic's published system prompt history for Claude is transformed into a git-based exploration tool, breaking up the monolithic markdown source into granular files and timestamped commits. By structuring extracted prompts per model, family, and revision, researchers can leverage `git log`, `diff`, and `blame` to trace prompt evolution, compare differences, and attribute changes to specific dates—all without manual parsing. The extraction workflow uses precise commit metadata to preserve chronology and clarity, enabling reproducible and detailed investigations of prompt adjustments across Opus, Sonnet, and Haiku model families. Access to both the original [Anthropic system prompts](https://platform.claude.com/docs/en/release-notes/system-prompts) and [prompt histories on GitHub](https://github.com/simonw/research/tree/main/extract-system-prompts) facilitates transparent, permalinks, and fine-grained audit trails.

Key features:
- 26 prompt revisions across 14 models and three model families
- Four artifact types per revision, including per-model, per-family, and firehose files
- Git commit timestamps and authors reflect the original prompt dates for historical accuracy
- Idempotent extraction with reproducible results for future updates
