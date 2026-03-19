# REXC (rx) JSON Test Suite — Investigation Notes

## What I Did

### 1. Cloned and explored the rx repo
- Repo: https://github.com/creationix/rx
- REXC is a compact binary-ish encoding format for JSON-like data
- Uses base64 numeric system, zigzag encoding for signed integers, and various tag bytes
- Features: pointers (dedup), schemas (shared object shapes), chains (path splitting), indexes (O(1) access)
- Implementation is in TypeScript, tests use vitest/bun

### 2. Ran existing tests
- 294 tests across 2 files (rx.test.ts and b64.test.ts), all passing
- Tests cover: b64 encode/decode, zigzag, read/write primitives, containers, pointers, chains, schemas, proxy API, inspect API, round-trips

### 3. Created JSON test suite
- Extracted 206 tests into `rx-tests.json` covering:
  - `b64_stringify`: 22 tests for integer → base64 string
  - `b64_parse`: 25 tests for base64 string → integer
  - `zigzag_encode`: 13 tests for signed → unsigned zigzag
  - `zigzag_decode`: 13 tests for unsigned → signed zigzag
  - `stringify`: 36 tests for value → rexc string encoding
  - `parse`: 22 tests for rexc string → value decoding
  - `roundtrip`: 63 tests for encode→decode identity
  - `split_number`: 12 tests for number decomposition
- Used `{"__special": "..."}` wrappers for NaN, Infinity, -Infinity, undefined

### 4. TypeScript test runner
- Created `rx-json.test.ts` that loads the JSON file and runs all tests against the original implementation
- Uses vitest, same as the original test suite
- All 206 tests pass

### 5. Python implementation
- Built from scratch by reading the TypeScript source
- Key implementation details:
  - Python doesn't have unsigned right shift, needed manual masking for 32-bit zigzag
  - Used `UNDEFINED` singleton for JavaScript's undefined
  - Cursor-based parser like the original
  - Critical bug fix: cursor reuse in decoder — `wrap()` follows pointers which mutates cursor position, so need separate cursor instances for iteration vs. value resolution
  - `split_number` uses Python's string formatting to match TypeScript's toPrecision(14) behavior

### Learned Along the Way
- The REXC format is quite clever — right-to-left layout means the root node is at the end
- Pointers use byte offsets as deltas, making dedup very space-efficient
- Schema objects separate keys from values, with shared key lists across objects
- The chain mechanism splits strings at delimiters and deduplicates shared prefixes
- Python's arbitrary precision integers make zigzag simpler in some ways (no overflow) but need careful handling of the 32-bit boundary
