# REXC (rx) JSON Test Suite

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A language-agnostic JSON test suite for the [REXC](https://github.com/creationix/rx) encoder/decoder, plus a complete Python implementation tested against it.

## What's Here

### `rx-tests.json` — The Test Suite

A single JSON file containing 206 tests organized into sections:

| Section | Tests | Description |
|---------|-------|-------------|
| `b64_stringify` | 22 | Integer → base64 string encoding |
| `b64_parse` | 25 | Base64 string → integer decoding |
| `zigzag_encode` | 13 | Signed → unsigned zigzag encoding |
| `zigzag_decode` | 13 | Unsigned → signed zigzag decoding |
| `stringify` | 36 | Value → REXC string encoding |
| `parse` | 22 | REXC string → value decoding |
| `roundtrip` | 63 | Encode→decode identity tests |
| `split_number` | 12 | Number decomposition into base×10^exp |

Special values (NaN, Infinity, undefined) are represented as `{"__special": "NaN"}` etc.

### `rx-json.test.ts` — TypeScript Test Runner

Loads `rx-tests.json` and runs all tests against the original TypeScript rx implementation using vitest. Validates that the JSON test suite matches the reference implementation.

### `rx-python/` — Python Implementation

A complete Python port of the REXC encoder/decoder, created with `uv init` and tested with pytest.

#### Running the Python tests

```bash
cd rx-python
uv run pytest -v
```

#### What's implemented

- **b64**: Base64 numeric system (encode, decode, read, write)
- **zigzag**: Signed↔unsigned integer encoding
- **split_number**: Float decomposition for compact encoding
- **stringify/encode**: Full REXC encoder with pointers, schemas, chains, and indexes
- **parse/decode**: Full REXC decoder handling all node types
- All 206 JSON test suite tests pass

## Running the TypeScript tests

The TypeScript test file is designed to be placed alongside the rx repo:

```bash
cd /tmp/rx
# Copy rx-json.test.ts into the repo
bun test rx-json.test.ts
```
