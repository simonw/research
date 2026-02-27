# Unicode Explorer — Binary Search Over HTTP

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A demo that performs **binary search via HTTP Range requests** against a single static file. No backend, no database, no dependencies. Every step of the binary search is a real network fetch — the browser reads one 256-byte record at a time, compares, narrows the range, and fetches again.

Try it out here: https://tools.simonwillison.net/unicode-binary-search

## How It Works

1. **`build.py`** downloads Unicode character data and produces a 76MB binary file where every character is a fixed-width 256-byte JSON record, sorted by codepoint
2. **`index.html`** (zero dependencies) performs binary search by fetching individual records via HTTP Range requests
3. A **network log panel** shows each step in real time — signpost lookups appear instantly as cached, then Range fetches appear one by one

The fixed-width records are what make this work: `byte_offset = record_index × 256`. No index file needed — just arithmetic.

## Architecture

### Data File (`data/unicode-data.bin`)

Each of the 299,382 records is exactly 256 bytes:
```json
{"cp":65,"name":"LATIN CAPITAL LETTER A","cat":"Lu","block":"Basic Latin"}
```
Right-padded with spaces to fill the 256-byte slot. Records are sorted by codepoint.

### Signposts

The first 3 steps of binary search over ~300k records always hit the same indices (the ½, ¼, ¾ points). The build script samples 8 codepoints at evenly-spaced ⅛-point positions and includes them in `meta.json`. The client walks these to narrow the search space to a ⅛th segment before making any network requests, saving ~3 round trips.

### Search Flow

1. Fetch `meta.json` on page load (record width, total count, signposts)
2. Parse input (character via `codePointAt(0)` or hex like `2665` / `U+2665`)
3. Walk signposts to find tightest bounds (no network)
4. Loop: `fetch("data/unicode-data.bin", { headers: { Range: "bytes=N-M", "Accept-Encoding": "identity" } })`
5. Parse the 256-byte record, compare codepoint, narrow `lo`/`hi`
6. Repeat until found or exhausted

Each fetch transfers exactly 256 bytes. A typical search takes ~15 Range requests.

## Running

```bash
# One-time build (Python 3, no pip packages needed)
python3 build.py

# Serve with any static server that supports Range requests
npx serve .
```

Then open http://localhost:3000 and search for a character.

## Example

Searching for 💀 (U+1F480 SKULL):

| Step | Source | Record | Comparison | Time |
|------|--------|--------|------------|------|
| 1 | signpost | U+0000 | 128128 > 0 → right | cached |
| 2 | signpost | U+9634 | 128128 > 38452 → right | cached |
| 3 | signpost | U+14026 | 128128 > 81958 → right | cached |
| 4 | signpost | U+24B21 | 128128 < 150305 → left | cached |
| 5 | bytes=23950336-23950591 | U+20209 | 128128 < 131593 → left | 11ms |
| ... | ... | ... | ... | ... |
| 19 | bytes=23366144-23366399 | U+1F480 SKULL | Found! | 26ms |

**Found in 19 steps (4 cached, 15 fetched) · 3,840 bytes transferred · full file is 73.1MB**

## Key Constraints

- The client **never** downloads the full data file — every access is a single-record Range request
- Binary search happens **over the network**, not in memory (except signpost pre-narrowing)
- Range requests use `Accept-Encoding: identity` to prevent compression from breaking byte offsets
- Zero external dependencies in the client (vanilla HTML/CSS/JS)
- Build script uses Python standard library only
