Leveraging [ast-grep](https://ast-grep.github.io/) and custom YAML rules, the AST-Grep Import Rewriter offers a structured approach to automatically extract, analyze, and rewrite obfuscated JavaScript import statements across ES6, CommonJS, dynamic imports, and webpack bundles. By parsing source files, it generates mapping templates and applies user-defined mappings, converting unreadable module paths into meaningful names with either regex- or AST-based transformations. Featuring a command-line interface, the tool integrates with Python and ast-grep CLI, ensuring accurate code rewriting and comprehensive import discovery. Limitations include restricted support for runtime-evaluated imports and complex obfuscations, but the workflow simplifies code cleanup and migration in modern JS projects.

Key features:
- Supports multiple import styles (ES6, CommonJS, dynamic, webpack, re-exports).
- Generates and applies JSON mapping files for path deobfuscation.
- Provides both regex and AST-guided transformation options.
- Includes rules and config for direct use with [ast-grep scan](https://ast-grep.github.io/guide/cli.html#scan-command).
- Handles webpack-specific patterns automatically.
