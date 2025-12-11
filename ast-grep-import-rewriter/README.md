# AST-Grep Import Rewriter

A tool that uses [ast-grep](https://ast-grep.github.io/) to parse JavaScript files and rewrite obfuscated module import references based on a mapping file.

## Overview

This prototype demonstrates how to use ast-grep for structural code analysis and transformation, specifically targeting obfuscated JavaScript module imports. It can:

- **Extract** all import statements from JavaScript files (ES6, CommonJS, dynamic imports, webpack bundles)
- **Generate** mapping templates for obfuscated module paths
- **Rewrite** obfuscated import paths to readable names

## Installation

### Prerequisites

1. **ast-grep CLI** (version 0.40.1 or higher):
   ```bash
   npm install -g @ast-grep/cli
   ```

2. **Python 3.10+** (for the main rewriter script)

### Setup

```bash
git clone <this-repo>
cd ast-grep-import-rewriter
```

## Usage

### Extract Imports

View all imports in a JavaScript file:

```bash
python import_rewriter.py <file.js> --extract

# JSON output for programmatic use
python import_rewriter.py <file.js> --extract --format json
```

Example output:
```
Found 16 imports in test-samples/obfuscated-sample.js:

  Line 3: [es6] ./mod_0x3c4d
    import { _0x1a2b as foo } from "./mod_0x3c4d";

  Line 13: [commonjs] ./legacy_0xaaaa
    require("./legacy_0xaaaa")

  Line 25: [webpack] ./src/utils_0xc3d4.js
    __webpack_require__("./src/utils_0xc3d4.js")
```

### Generate Mapping Template

Automatically generate a mapping template for obfuscated imports:

```bash
python import_rewriter.py <file.js> --generate-mapping > mapping.json

# Custom pattern for obfuscation detection (default: _0x[a-f0-9]+)
python import_rewriter.py <file.js> --generate-mapping --pattern "chunk_[0-9a-f]+"
```

### Rewrite Imports

Apply a mapping to rewrite obfuscated imports:

```bash
# Dry run (preview changes)
python import_rewriter.py <file.js> --mapping mapping.json --dry-run

# Write to new file
python import_rewriter.py <file.js> --mapping mapping.json --output clean.js

# Modify in place
python import_rewriter.py <file.js> --mapping mapping.json
```

### Use AST-Based Rewriting

For more precise AST-aware transformations:

```bash
python import_rewriter.py <file.js> --mapping mapping.json --use-ast --dry-run
```

## Mapping File Format

The mapping file is a simple JSON object mapping obfuscated paths to readable names:

```json
{
  "./mod_0x3c4d": "./modules/user-auth",
  "./util_0x7g8h": "./utils/helpers",
  "./lib_0xabcd": "./lib/core",
  "./src/api_0xe5f6.js": "./src/api.js"
}
```

## Supported Import Types

| Type | Pattern | Example |
|------|---------|---------|
| ES6 Default | `import x from "..."` | `import React from "react"` |
| ES6 Named | `import { x } from "..."` | `import { useState } from "react"` |
| ES6 Namespace | `import * as x from "..."` | `import * as utils from "./utils"` |
| CommonJS | `require("...")` | `const fs = require("fs")` |
| Dynamic | `import("...")` | `const mod = await import("./lazy")` |
| Re-exports | `export { x } from "..."` | `export { foo } from "./bar"` |
| Export All | `export * from "..."` | `export * from "./helpers"` |
| Webpack | `__webpack_require__("...")` | `__webpack_require__("./module")` |

## AST-Grep Rules

The `rules/` directory contains YAML rule definitions for ast-grep's `scan` command:

```bash
# Use ast-grep directly with rules
cd ast-grep-import-rewriter
ast-grep scan test-samples/obfuscated-sample.js
```

Available rules:
- `es6-import-default.yml` - ES6 default/named imports
- `es6-import-named.yml` - ES6 named imports specifically
- `es6-import-namespace.yml` - ES6 namespace imports
- `commonjs-require.yml` - CommonJS require()
- `dynamic-import.yml` - Dynamic import()
- `export-from.yml` - Re-exports

## Examples

### Basic Workflow

1. **Analyze** the obfuscated file:
   ```bash
   python import_rewriter.py obfuscated-bundle.js --extract
   ```

2. **Generate** initial mapping:
   ```bash
   python import_rewriter.py obfuscated-bundle.js --generate-mapping > mapping.json
   ```

3. **Edit** the mapping file to add meaningful names

4. **Preview** the changes:
   ```bash
   python import_rewriter.py obfuscated-bundle.js --mapping mapping.json --dry-run
   ```

5. **Apply** the transformation:
   ```bash
   python import_rewriter.py obfuscated-bundle.js --mapping mapping.json --output clean-bundle.js
   ```

### Webpack Bundle Analysis

For webpack bundles with obfuscated chunk names:

```bash
# Extract all imports including webpack-specific patterns
python import_rewriter.py webpack-bundle.js --extract

# The tool detects __webpack_require__ calls automatically
```

## Architecture

```
ast-grep-import-rewriter/
├── import_rewriter.py     # Main CLI tool
├── sgconfig.yml           # ast-grep configuration
├── rules/                 # YAML rule definitions
│   ├── es6-import-default.yml
│   ├── es6-import-named.yml
│   ├── es6-import-namespace.yml
│   ├── commonjs-require.yml
│   ├── dynamic-import.yml
│   └── export-from.yml
└── test-samples/          # Example files for testing
    ├── obfuscated-sample.js
    ├── webpack-bundle-sample.js
    ├── mapping.json
    └── webpack-mapping.json
```

## Limitations

- Template literals in imports (e.g., `` require(`./mod_${var}`) ``) are not supported as they require runtime evaluation
- AMD-style `define()` calls are detected by text replacement but not by ast-grep patterns (requires custom rule)
- Very heavily obfuscated code with computed module IDs may need manual analysis

## How It Works

1. **Pattern Matching**: Uses ast-grep's pattern syntax with metavariables (`$VAR`, `$$$VAR`) to match import statements
2. **JSON Output**: ast-grep's `--json` flag provides structured match data including positions
3. **Rewriting**: Two modes available:
   - **Regex-based**: Simple string replacement (fast, handles all cases)
   - **AST-based**: Uses ast-grep's rewrite feature for precise transformations

## References

- [ast-grep Documentation](https://ast-grep.github.io/)
- [ast-grep Pattern Syntax](https://ast-grep.github.io/guide/pattern-syntax.html)
- [Tree-sitter JavaScript Grammar](https://github.com/tree-sitter/tree-sitter-javascript)
