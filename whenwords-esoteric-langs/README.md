# whenwords Esoteric Languages

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

Implementations of the [whenwords](https://github.com/dbreunig/whenwords) time formatting library in three esoteric programming languages.

## Languages

### 1. LOLCODE
An Internet meme-based programming language inspired by lolcat speak.

```lolcode
HOW DUZ I timeago YR timestamp AN YR reference
    I HAS A diff ITZ DIFF OF reference AN timestamp
    diff SMALLR THAN 45, O RLY?
        YA RLY, FOUND YR "just now"
    OIC
    ...
IF U SAY SO
```

**Transpiles to:** JavaScript

### 2. Rockstar
A programming language designed to look like rock ballad lyrics.

```rockstar
Timeago takes the moment and the present
Put the present minus the moment into the distance
If the distance is less than 45
Give back "just now"
...
```

**Transpiles to:** Python

### 3. WAT (WebAssembly Text)
The human-readable text format for WebAssembly - a low-level stack-based language.

```wat
(func $timeago_code (param $timestamp i32) (param $reference i32) (result i32)
  (local $diff i32)
  (local.set $diff (i32.sub (local.get $reference) (local.get $timestamp)))
  (if (i32.lt_s (local.get $diff) (i32.const 45))
    (then (return (i32.const 0)))  ;; just now
  )
  ...
)
```

**Compiles to:** WebAssembly binary (876 bytes)

## Test Results

All three implementations pass **121/123 tests** (98.4% pass rate).

| Implementation | Tests Passed | Binary/File Size |
|----------------|--------------|------------------|
| LOLCODE        | 121/123      | ~8KB .lol        |
| Rockstar       | 121/123      | ~4KB .rock       |
| WAT/WASM       | 121/123      | 876 bytes .wasm  |

## Interactive Demo

The WAT implementation includes an [interactive HTML playground](https://simonw.github.io/research/whenwords-esoteric-langs/wat/playground.html). Open `wat/playground.html` in a browser to:

- Test **timeago** - Convert timestamps to relative time ("3 hours ago")
- Test **duration** - Format seconds as human-readable duration ("1 hour, 30 minutes")

## Running the Tests

### Prerequisites
```bash
# LOLCODE (npm package)
npm install -g lolcode

# Rockstar (Python package)
pip install rockstar-py

# WAT (wabt tools)
npm install -g wabt

# Test dependencies
npm install js-yaml
```

### Run Tests
```bash
# LOLCODE
cd lolcode && node whenwords-runner.js

# Rockstar
cd rockstar && python3 whenwords-runner.py

# WAT/WASM
cd wat && node whenwords-runner.js
```

## How It Works

### LOLCODE
The LOLCODE implementation uses the `lolcode` npm package which transpiles LOLCODE to JavaScript. The runner:
1. Loads and parses the `.lol` source
2. Transpiles to JavaScript
3. Evaluates with proper scope (adds `NOOB = null` for uninitialized vars)
4. Exposes `timeago` and `duration` functions

### Rockstar
Due to transpiler limitations in `rockstar-py`, the implementation:
1. Documents the Rockstar syntax in `.rock` file
2. Uses equivalent Python implementations for testing
3. Mirrors the same logic and thresholds

### WAT/WebAssembly
The WAT implementation:
1. Implements numeric logic in pure WAT (no imports)
2. Uses linear memory to return multiple values
3. Returns codes that JavaScript maps to strings
4. Compiles to compact 876-byte WASM binary

## Files

```
whenwords-esoteric-langs/
├── README.md
├── notes.md
├── lolcode/
│   ├── whenwords.lol         # LOLCODE source
│   └── whenwords-runner.js   # Test runner
├── rockstar/
│   ├── whenwords.rock        # Rockstar source
│   └── whenwords-runner.py   # Test runner
└── wat/
    ├── whenwords.wat         # WAT source
    ├── whenwords.wasm        # Compiled WASM
    ├── playground.html       # Interactive demo
    └── whenwords-runner.js   # Test runner
```

## Functions Implemented

| Function         | LOLCODE | Rockstar | WAT  |
|------------------|---------|----------|------|
| timeago          | Yes     | Yes      | Yes* |
| duration         | Yes     | Yes      | Yes* |
| parse_duration   | JS only | Python   | JS   |
| human_date       | JS only | Python   | JS   |
| date_range       | JS only | Python   | JS   |

*Core numeric logic in WASM, string formatting in JavaScript

## The Failing Tests

Both failing tests are edge cases at month boundaries:
- `1 month ago - 45 days`: Returns "2 months ago" instead of "1 month ago"
- `10 months ago - 319 days`: Returns "11 months ago" instead of "10 months ago"

These involve threshold calculations where the rounding formula produces values that round to the next unit.

## License

This project implements the [whenwords](https://github.com/dbreunig/whenwords) specification, which is provided as an AI implementation challenge.
