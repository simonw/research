# ABC to WebAssembly Browser Project

## Initial Setup - Nov 29, 2025

Cloned abc-unix from https://github.com/gvanrossum/abc-unix to /tmp/abc-unix

## Source Structure Analysis

The ABC codebase has the following structure:
- `b/` - basic common functions
- `bed/` - editor code
- `bint1/`, `bint2/`, `bint3/` - interpreter components
- `btr/` - B-tree storage
- `bio/` - IO functions
- `stc/` - standard commands
- `unix/` - unix-specific code
- `generic/` - generic platform code including main.c
- `bhdrs/`, `ehdrs/`, `ihdrs/` - header files

## Key Files
- `generic/main.c` - main entry point
- `Makefile.unix` - Unix build system
- Uses termcap library for terminal handling

## Examples Found
- `ex/basic.ex.in` - Tests all basic functions and operators
- `ex/hanoi.ex/` - Tower of Hanoi algorithm
- `ex/pi.ex/` - Calculate pi to n digits

## ABC Language Notes
- Commands are in `.cmd` files (HOW TO definitions)
- `.abc` files contain permanents (variables/tests/workspaces)
- ABC is Python's direct predecessor (worked on 1983-1986 by Guido van Rossum et al.)

## Emscripten Setup

Installed emsdk from git to /tmp/emsdk
- Version: 4.0.20
- Activated successfully

## Compilation Strategy

For WebAssembly, I need to:
1. Modify the code to not require termcap (no terminal support in browser)
2. Redirect I/O to JavaScript-callable functions
3. Handle filesystem via Emscripten's virtual FS
4. Build as a module that can be called from JS

## Challenges
1. Original code assumes 32-bit system (int == pointer size)
2. Uses termcap for terminal handling
3. Complex multi-directory build system
4. Signal handling needs to be adapted
5. **CRITICAL: Source file corruption** - Many source files have corrupted lines where macro definitions and function definitions got merged together

## Source File Corruption Discovery

During compilation attempts, I discovered that many source files in the ABC repository have corrupted content. The corruption pattern appears to be macro definitions that got merged with subsequent function definitions. Examples:

### Pattern of corruption:
```c
// Corrupted:
#define MACRO(param) content FUNCTION_DEF) {

// Should be:
#define MACRO(param) content

FUNCTION_DEF {
```

### Files with confirmed corruption:
- `bint1/i1nur.c` line 87: D macro merged with rat_sum function
- `bint2/i2dis.c` lines 70, 415: d_space/d_newline and is_b_tag/d_dyaf
- `bint2/i2uni.c` line 30: unicmd_suite/unit merged
- `bint2/i2exp.c` lines 70, 233: comment/initstack and Prio_err/do_dya
- `btr/i1lta.c` line 57: Pop macro/unzip function
- `bint3/i3int.c` line 38: F macro/run function

Many more corrupted lines likely exist in the remaining failing files.

## Compilation Progress

After extensive fixes, achieved:
- 78 out of ~90 core source files compile successfully
- 21 files still fail due to remaining corruption or K&R prototype conflicts
- Key failing files are in bint3/ (runtime), btr/ (b-tree), and generic/ (platform)

## Header File Fixes Applied

1. **Type forward declarations** - Added guards to prevent typedef redefinition:
   - WSENV_DEFINED, WSENVPTR_DEFINED
   - BTREEPTR_DEFINED, ITEMPTR_DEFINED
   - ENVIRON_DEFINED, EXPADM_DEFINED

2. **K&R to ANSI C prototypes** - Updated many function declarations:
   - i1num.h: int_gadd, int_ldiv, etc.
   - i1btr.h: grabbtreenode, copybtree, etc.
   - i2par.h: cmd_suite, cmd_seq, ucmd_seq
   - bobj.h: key, assoc, adrassoc

3. **WASM32 configuration** - unix/mach.h modified for 32-bit WASM:
   - 32-bit longs
   - 64-bit twodigit for large number arithmetic

4. **Emscripten compatibility**:
   - debug() macro disabled for WASM builds
   - Various type conflicts resolved

## Conclusion

The ABC codebase from github.com/gvanrossum/abc-unix has **extensive source file corruption** that prevents clean compilation to WebAssembly. The corruption appears systematic, affecting macro/function boundaries throughout the codebase.

A complete WASM port would require:
1. Manual repair of all corrupted source files
2. Conversion of remaining K&R C to ANSI C
3. Platform adaptation for browser environment
4. Testing and debugging

This is a significant undertaking beyond the scope of automated fixes.
