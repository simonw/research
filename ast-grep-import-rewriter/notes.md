# AST-Grep Import Rewriter - Development Notes

## Initial Setup
- Date: 2025-12-11
- Goal: Build a tool using ast-grep to parse obfuscated JavaScript and rewrite module import references
- ast-grep is installed as `sg` command

## Research Phase

### Understanding ast-grep
- ast-grep (sg) is a CLI tool for structural code search and manipulation
- Uses tree-sitter for parsing
- Supports pattern matching and rewriting

### Key Concepts
1. **Pattern Matching**: Use `$VAR` for metavariables that match AST nodes
2. **Rewriting**: Can replace matched patterns with new code
3. **Rules**: YAML-based configuration for complex transformations

## Development Log

### 2025-12-11: Initial Investigation

**ast-grep Installation**
- Initially thought `/usr/bin/sg` was ast-grep, but it's actually the `sg` command for groups
- Installed ast-grep via npm: `npm install -g @ast-grep/cli`
- Version installed: 0.40.1

**Pattern Matching Discovery**
Learned several key patterns that work well for JavaScript imports:

1. **ES6 imports**: `import $NAME from "$SOURCE"`
   - Matches default imports, named imports, namespace imports
   - The `$NAME` metavariable captures the import clause
   - `$$$ALL` matches multiple items (like in named imports)

2. **CommonJS require**: `require("$SOURCE")`
   - Simple pattern that captures the module path

3. **Dynamic imports**: `import("$SOURCE")`
   - Matches ES2020 dynamic imports

4. **Webpack bundler patterns**: `__webpack_require__("$SOURCE")`
   - Important for analyzing bundled code

**Quote Handling**
- Need separate patterns for single vs double quotes
- ast-grep treats them as different string types

**JSON Output**
- Use `--json` flag to get structured output
- Output includes line/column positions and matched text

**Rewriting**
- Use `--rewrite` flag with metavariable references
- Produces diff-like output showing changes
- Use `--update-all` to apply changes in-place

### Challenges Encountered

1. **Duplicate Detection**: The default import pattern also matches named imports and namespace imports. Had to track seen positions to avoid duplicates.

2. **Multi-line Imports**: ast-grep handles multi-line imports correctly as a single match.

3. **Webpack Bundles**: Standard import patterns don't match `__webpack_require__` calls. Had to add explicit patterns for these.

### Key Learnings

- ast-grep is powerful for structural code manipulation
- YAML rules allow for complex rule definitions with multiple patterns
- The `scan` command with rules is useful for batch analysis
- The `run` command is better for one-off searches and rewrites
- Metavariable `$$$VAR` captures multiple items (like spread operator)
- For comprehensive analysis, need multiple patterns covering all import styles

