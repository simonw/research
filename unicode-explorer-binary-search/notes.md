# Unicode Explorer — Binary Search Over HTTP

## Notes

### Plan
- Build a Python script (`build.py`) that downloads UnicodeData.txt and Blocks.txt, produces a fixed-width binary file and meta.json
- Build a single-file HTML client (`index.html`) that performs binary search via HTTP Range requests
- Each record is 256 bytes, JSON, right-padded with spaces
- Signposts at 1/8th intervals skip first 3 binary search steps
- Network log UI shows the search happening in real time

### Build Script
- Downloads UnicodeData.txt (2.2MB) and Blocks.txt (11KB) from unicode.org
- Parses 346 blocks for block name lookup
- Includes control characters (using Unicode 1.0 names from field 10)
- Expands CJK/Hangul/Tangut ranges which are listed as First/Last in UnicodeData.txt
- Each record: `{"cp":65,"name":"LATIN CAPITAL LETTER A","cat":"Lu","block":"Basic Latin"}`
- 256 bytes per record, right-padded with spaces, sorted by codepoint
- Names truncated if record would exceed 256 bytes
- Total: 299,382 records = 76.6MB binary file

### Bug Fix: Input Parsing
- Initial version treated single characters like 'A' as hex (0xA = 10 = LINE FEED)
- Fixed by: only parse as hex if it has a prefix (U+, 0x, \u) or is 4+ hex digits
- Characters (1-2 codepoints) are always treated as literal characters via codePointAt(0)

### Signposts
- 8 signposts at evenly-spaced 1/8th intervals of the record array
- Walking them narrows the search space before any network requests
- Saves ~3 round trips per search
- Displayed as greyed-out "cached" rows in the network log

### Testing
- Tested with rodney (Chrome automation)
- 'A' → LATIN CAPITAL LETTER A (17 steps, 2 cached + 15 fetched)
- '2665' → BLACK HEART SUIT (16 steps, 2 cached + 14 fetched)
- '1F480' → SKULL (19 steps, 4 cached + 15 fetched)
- 'U+FEFF' → ZERO WIDTH NO-BREAK SPACE (18 steps, 3 cached + 15 fetched)
- Range requests use Accept-Encoding: identity to prevent compression
