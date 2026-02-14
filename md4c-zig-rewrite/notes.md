# md4c Zig Rewrite - Development Notes

## Goal
Rewrite the md4c markdown parser entirely in Zig, getting 100% of conformance & regression tests to pass.

## Approach

### Phase 1: Build Infrastructure
- Cloned md4c from https://github.com/mity/md4c (commit at HEAD)
- Downloaded Zig 0.13.0 for linux-x86_64
- Built original C code with CMake to establish test baseline
- All tests pass: 652 spec + 61 regressions + 19 coverage + 73 extension tests + 29 pathological = 833 total

### Phase 2: Zig Build System
- Created `build.zig` to compile C source files using Zig's native C compiler
- Verified 100% test pass with Zig-compiled binary
- This confirmed the build infrastructure works

### Phase 3: Port md2html CLI to Zig
- Wrote `src/main.zig` - full reimplementation of md2html.c in pure Zig
- Handles all command-line flags (--ftables, --fstrikethrough, --fpermissive-url-autolinks, etc.)
- Reads from stdin/file, writes HTML to stdout/file
- 100% test pass confirmed

### Phase 4: Port entity.c to Zig
- Generated `src/entity.zig` from the C entity table using a Python extraction script
- Contains 2,125 HTML entity definitions with Unicode codepoint mappings
- Implements binary search with C ABI export (`entity_lookup`)
- 100% test pass confirmed

### Phase 5: Port md4c-html.c to Zig
- Wrote `src/md4c_html.zig` - faithful translation of the HTML renderer
- Implements all callback functions (enter_block, leave_block, enter_span, leave_span, text)
- Handles HTML escaping, URL escaping, utf-8 codepoint rendering, entity resolution
- Exports `md_html` with C ABI
- Notable: `align` is a Zig keyword, needed `@"align"` to access the struct field
- 100% test pass confirmed

### Phase 6: Port md4c.c (Core Parser) to Zig
This was the most complex phase - translating the 6,492-line core parser to Zig.

#### Strategy
1. **Initial translation**: Used `zig translate-c` on md4c.c to auto-translate 85 of 114 functions
2. **Python build script**: Assembled the translate-c output into a compilable md4c.zig file
3. **Manual translation**: 29 functions containing C `goto` statements couldn't be auto-translated
4. **Parallel agent translation**: Launched parallel agents to translate the 29 complex functions from C to Zig

#### Key Translation Challenges

**C `goto abort` pattern**:
The C code heavily uses `goto abort` for error handling. In Zig, this was translated to labeled blocks:
```zig
abort: {
    ret = some_call();
    if (ret < 0) break :abort;
    // ... more operations ...
    break :abort;
}
return ret;
```

**C `CHECK()` macro**:
Translated to inline checks: `ret = call(); if (ret < 0) break :abort;`

**Pointer alignment**:
C void pointer casts like `(MD_BLOCK*)ptr` required `@alignCast` in Zig:
`@as(*MD_BLOCK, @ptrCast(@alignCast(ptr)))`

**Optional pointer arithmetic**:
C code freely does arithmetic on pointers that may be NULL. In Zig, optional pointers (`?*T`) need to be cast to `[*c]T` before arithmetic:
`@as([*c]MD_CONTAINER, @ptrCast(@alignCast(ctx.*.containers))) + offset`

**Bitfield structs**:
C bitfield structs like `MD_BLOCK` with `unsigned type:8; unsigned flags:8; unsigned data:16;` were translated to a single `type_flags_data: u32` field with getter/setter methods.

**Reserved identifiers**:
C variables named `i6` and `u6` shadow Zig primitive types. Used escaped identifiers: `@"i6"`, `@"u6"`.

**Stale pointer after realloc**:
The most subtle bug: `md_analyze_line` cached `ctx->containers` as a local pointer, but `md_push_container()` could call `realloc()`, invalidating it. The C code accesses `ctx->containers` directly each time; the Zig translation cached it. Fixed by refreshing the pointer after `md_push_container()`.

**libc function access**:
Zig 0.13.0 doesn't expose `std.c.memset/memcpy/etc.` directly. Used `extern fn` declarations instead:
```zig
extern fn memcpy(dest: ?*anyopaque, src: ?*const anyopaque, n: usize) ?*anyopaque;
extern fn memset(dest: ?*anyopaque, c: c_int, n: usize) ?*anyopaque;
```

**HTML block detection tags**:
The C code uses static arrays of TAG structs for HTML block type detection. These were re-implemented as Zig comptime-friendly const arrays with a `makeTag` helper function.

#### Functions Translated Manually (29 total)
The following functions contained C `goto` statements and required manual translation:
- `md_process_doc` - Main document processing loop
- `md_analyze_line` - Line type analysis (~700 lines, the largest function)
- `md_collect_marks` - Inline mark collection (~800 lines)
- `md_process_line` - Line processing dispatch
- `md_process_all_blocks` - Block output processing
- `md_process_leaf_block` - Leaf block handling
- `md_process_table_block_contents` - Table block handling
- `md_process_table_row` - Table row handling
- `md_process_table_cell` - Table cell handling
- `md_process_verbatim_block_contents` - Code block handling
- `md_setup_fenced_code_detail` - Fenced code detail setup
- `md_analyze_inlines` - Inline analysis
- `md_process_inlines` - Inline processing
- `md_process_normal_block_contents` - Normal block content processing
- `md_is_inline_link_spec` - Inline link detection
- `md_enter_leave_span_a` - Link/image span handling
- `md_enter_leave_span_wikilink` - Wiki link span handling
- `md_end_current_block` - Block termination
- `md_push_container_bytes` - Container byte pushing
- `md_enter_child_containers` - Child container entry
- `md_leave_child_containers` - Child container exit
- `md_is_html_block_start_condition` - HTML block start detection (fully rewritten)
- `md_is_html_block_end_condition` - HTML block end detection (fully rewritten)
- And several others from the inline/block processing pipeline

## Test Results

All test suites pass at 100%:

| Test Suite | Tests | Pass |
|---|---|---|
| spec.txt (CommonMark) | 652 | 652 |
| regressions.txt | 61 | 61 |
| coverage.txt | 19 | 19 |
| spec-tables.txt | 12 | 12 |
| spec-wiki-links.txt | 23 | 23 |
| spec-permissive-autolinks.txt | 15 | 15 |
| spec-latex-math.txt | 6 | 6 |
| spec-strikethrough.txt | 5 | 5 |
| spec-tasklists.txt | 5 | 5 |
| spec-underline.txt | 4 | 4 |
| spec-hard-soft-breaks.txt | 2 | 2 |
| pathological inputs | 29 | 29 |
| **Total** | **833** | **833** |

## Key Learnings

1. `zig translate-c` handles ~75% of C code automatically but fails on `goto` statements
2. C `goto abort` error handling maps well to Zig's labeled block `break :abort` pattern
3. Pointer alignment casts (`@alignCast`) are required everywhere C does void pointer casts
4. Optional pointers in Zig (`?*T`) need explicit unwrapping before dereference
5. Cached pointers become stale after `realloc` - the C code avoids this by re-reading from structs
6. Zig's `callconv(.C)` enables seamless callback interop with C APIs
7. C bitfield structs need manual getter/setter patterns in Zig
8. Some C identifiers (`i6`, `u6`, `align`) conflict with Zig keywords/primitives - use `@"name"` escaping
9. `extern fn` declarations work well for accessing libc functions
10. Zig's comptime evaluation works well for building static lookup tables (TAG arrays)
