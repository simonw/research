# md4c Zig Rewrite

A complete Zig rewrite of the [md4c](https://github.com/mity/md4c) Markdown parser, achieving **100% pass rate** on all 833 conformance, regression, and pathological tests.

## Overview

md4c is a fast CommonMark-compliant Markdown parser written in C (~9,200 lines across 3 source files). This project rewrites the entire codebase in Zig: the core parser (md4c.c, 6,492 lines), the HTML renderer (md4c-html.c), the entity lookup table (entity.c), and the md2html CLI tool.

### Components

| Component | Lines | Description |
|---|---|---|
| `src/md4c.zig` | 7,958 | Core CommonMark parser (114 functions) |
| `src/entity.zig` | 4,335 | HTML entity lookup (2,125 entities) |
| `src/md4c_html.zig` | 607 | HTML renderer with callbacks |
| `src/main.zig` | 435 | md2html CLI tool |
| `build.zig` | 55 | Zig build system |
| **Total** | **13,390** | **All Zig, zero C** |

The original C source is retained in `csrc/` for reference only and is not compiled.

## Building

Requires [Zig](https://ziglang.org/) 0.13.0+.

```bash
zig build
```

The built binary is at `zig-out/bin/md2html`.

## Usage

```bash
# Convert markdown file to HTML
echo "# Hello World" | zig-out/bin/md2html

# With GitHub Flavored Markdown extensions
echo "~~strikethrough~~" | zig-out/bin/md2html --fstrikethrough

# All extension flags
zig-out/bin/md2html --ftables --fstrikethrough --ftasklists --fwiki-links
```

### Supported Flags

**Extensions:**
- `--fcollapse-whitespace` -- Collapse non-trivial whitespace
- `--flatex-math` -- Enable LaTeX math spans
- `--fpermissive-atx-headers` -- Allow ATX headers without space
- `--fpermissive-url-autolinks` -- URL autolinks without `<>`
- `--fpermissive-www-autolinks` -- WWW autolinks without scheme
- `--fpermissive-email-autolinks` -- Email autolinks without `<>` and `mailto:`
- `--fpermissive-autolinks` -- All permissive autolinks
- `--fhard-soft-breaks` -- Force soft breaks as hard breaks
- `--fstrikethrough` -- Strikethrough with `~~`
- `--ftables` -- GitHub-style tables
- `--ftasklists` -- Task list items with `[x]`
- `--funderline` -- Underline with `_`
- `--fwiki-links` -- Wiki-style `[[links]]`

**Suppression:**
- `--fno-html-blocks` -- Disable raw HTML blocks
- `--fno-html-spans` -- Disable raw HTML spans
- `--fno-html` -- Disable all raw HTML
- `--fno-indented-code` -- Disable indented code blocks

**Dialects:**
- `--commonmark` -- CommonMark (default)
- `--github` -- GitHub Flavored Markdown

## Test Results

100% pass rate on all md4c test suites:

```
spec.txt:                    652 passed, 0 failed, 0 errored
regressions.txt:              61 passed, 0 failed, 0 errored
coverage.txt:                 19 passed, 0 failed, 0 errored
spec-tables.txt:              12 passed, 0 failed, 0 errored
spec-wiki-links.txt:          23 passed, 0 failed, 0 errored
spec-permissive-autolinks.txt: 15 passed, 0 failed, 0 errored
spec-latex-math.txt:           6 passed, 0 failed, 0 errored
spec-strikethrough.txt:        5 passed, 0 failed, 0 errored
spec-tasklists.txt:            5 passed, 0 failed, 0 errored
spec-underline.txt:            4 passed, 0 failed, 0 errored
spec-hard-soft-breaks.txt:     2 passed, 0 failed, 0 errored
pathological inputs:          29 passed, 0 failed, 0 errored
-------------------------------------------------------------
Total:                       833 passed, 0 failed, 0 errored
```

## Translation Approach

The translation used a hybrid strategy:

1. **`zig translate-c`** automatically translated 85 of 114 functions from md4c.c
2. **Manual translation** of 29 functions containing C `goto` statements, which `translate-c` cannot handle
3. The C `goto abort` error handling pattern was translated to Zig labeled blocks (`break :abort`)
4. C bitfield structs were translated to packed integer fields with getter/setter methods
5. All void pointer casts gained explicit `@alignCast` annotations required by Zig
6. C `extern fn` declarations provide access to libc functions (memcpy, memset, etc.)

## Architecture

```
md2html (main.zig)
  |
  v
md_html (md4c_html.zig) -- HTML renderer
  |
  v
md_parse (md4c.zig) -- Core CommonMark parser (114 functions)
  |
  v
entity_lookup (entity.zig) -- HTML entity resolution
```

All components are pure Zig with C ABI exports (`callconv(.C)`) for cross-module calls. The build system links them into a single static binary.

## License

The original md4c code is MIT licensed by Martin Mitas. This Zig rewrite maintains the same license.
