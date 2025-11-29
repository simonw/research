# ABC to WebAssembly Browser Port Investigation

## Overview

This project investigated compiling the ABC programming language interpreter to WebAssembly for browser execution. ABC is Python's direct predecessor, developed 1983-1986 at CWI Amsterdam by Leo Geurts, Lambert Meertens, and Steven Pemberton, with Guido van Rossum contributing to the Unix port.

## Repository

Source: https://github.com/gvanrossum/abc-unix

## Key Finding: Source File Corruption

**The ABC source repository contains extensive file corruption that prevents successful compilation.**

During the investigation, I discovered that many source files have corrupted lines where C preprocessor macro definitions and function definitions have been merged into single malformed lines. This corruption pattern appears throughout the codebase.

### Example of Corruption Pattern

Original corrupted code (from `bint1/i1nur.c` line 87):
```c
#define D(Denominator( u) Visible rational rat_sum(u, v) register rational u) {
```

Should be:
```c
#define D(u) Denominator(u)

Visible rational rat_sum(register rational u, register rational v) {
```

### Files with Confirmed Corruption

- `bint1/i1nur.c` - D macro + rat_sum function
- `bint2/i2dis.c` - d_space + d_newline, is_b_tag + d_dyaf
- `bint2/i2uni.c` - unicmd_suite + unit function
- `bint2/i2exp.c` - comment + initstack, Prio_err + do_dya
- `btr/i1lta.c` - Pop macro + unzip function
- `bint3/i3int.c` - F macro + run function

Many additional corrupted files remain.

## Compilation Attempts

### Environment
- Emscripten SDK 4.0.20 installed to /tmp/emsdk
- Target: WebAssembly (WASM32)
- Compiler flags: `-std=c89` for K&R C compatibility

### Results

After extensive manual repairs:
- **78 source files** compile successfully
- **21 source files** still fail due to remaining corruption or K&R prototype conflicts

### Failing Files
- `bed/e1erro.c`, `e1goto.c`, `e1node.c`, `e1sugg.c` (editor)
- `bint1/i1tra.c` (interpreter transformation)
- `bint2/i2fix.c`, `i2gen.c` (interpreter fixing/generation)
- `bint3/i3err.c`, `i3fil.c`, `i3fpr.c`, `i3loc.c`, `i3scr.c`, `i3sou.c` (runtime)
- `btr/i1obj.c`, `i1tex.c` (B-tree storage)
- `generic/file.c`, `os.c`, `trm.c` (platform layer)
- `stc/i2tca.c`, `i2tce.c`, `i2tcp.c` (standard commands)

## Modifications Made

See `abc-unix-wasm.diff` for the complete diff of changes made to the repository.

### Summary of Changes

1. **Header file guards** - Added typedef redefinition guards:
   - `WSENV_DEFINED`, `WSENVPTR_DEFINED`
   - `BTREEPTR_DEFINED`, `ITEMPTR_DEFINED`
   - `ENVIRON_DEFINED`, `EXPADM_DEFINED`

2. **K&R to ANSI C prototypes** - Updated function declarations in:
   - `ihdrs/i1num.h`
   - `btr/i1btr.h`
   - `ihdrs/i2par.h`
   - `bhdrs/bobj.h`

3. **WASM32 platform configuration** (`unix/mach.h`):
   - 32-bit WASM pointer/long configuration
   - 64-bit `twodigit` type for large number arithmetic

4. **Emscripten compatibility**:
   - `debug()` macro disabled for WASM builds
   - Type forward declaration alignment

5. **Source corruption repairs**:
   - Fixed 6+ corrupted macro/function definitions
   - Separated merged code blocks

## Conclusion

A complete WebAssembly port of ABC is **not achievable** with automated fixes due to the extent of source file corruption in the repository. A successful port would require:

1. Manual inspection and repair of all corrupted source files
2. Systematic conversion of K&R C function declarations to ANSI C
3. Implementation of browser-compatible I/O layer
4. Testing and debugging of the complete interpreter

## Files in This Directory

- `README.md` - This file
- `notes.md` - Detailed investigation notes
- `abc-unix-wasm.diff` - Git diff of all modifications made to abc-unix

## About ABC

ABC is a programming language designed as a replacement for BASIC. It pioneered many features that later appeared in Python:
- Indentation-based syntax
- Unlimited precision arithmetic
- Associative arrays (tables)
- Interactive environment

ABC was developed at CWI Amsterdam and significantly influenced Python's design.
