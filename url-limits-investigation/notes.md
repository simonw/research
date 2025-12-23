# URL Size Limits Investigation Notes

## Goal
Investigate maximum URL size limits enforced by Chromium, Firefox, and WebKit browser engines.

## Progress

### Step 1: Cloning repositories
- Cloned Chromium from https://github.com/chromium/chromium (commit: eae506cc8e9b1cd874a63d20d4d006a1428d29ec)
- Cloned Firefox from https://github.com/mozilla-firefox/firefox (commit: 20a1fb35a4d5c2f2ea6c865ecebc8e4bee6f86c9)
- Cloned WebKit from https://github.com/WebKit/WebKit (commit: 0a662721c2e04557824adff2324915aa0b5c8341)

### Step 2: Searching for URL limits

#### Chromium
- Found `kMaxURLChars` constant in `url/url_constants.h`
- Also defined in Mojo interface `url/mojom/url.mojom`
- Value: 2 * 1024 * 1024 = 2,097,152 bytes (2MB)
- URLs longer than this are silently replaced with empty/invalid URLs when passed over Mojo IPC

#### Firefox
- Multiple configurable preferences found in `modules/libpref/init/StaticPrefList.yaml`:
  - `network.standard-url.max-length`: 1,048,576 (1MB) - for standard URLs
  - `network.url.max-length`: 512 * 1024 * 1024 (512MB) - absolute maximum
  - `browser.history.maxUrlLength`: 2,000 chars - for history/bookmarks storage

#### WebKit
- No explicit URL-specific length limit found
- URLs are stored as WTF::String which has MaxLength = INT32_MAX (2,147,483,647 bytes, ~2GB)
- This means WebKit's URL parser doesn't enforce a specific URL length limit beyond string capacity

### Key Findings Summary
| Browser | URL Limit | Notes |
|---------|-----------|-------|
| Chromium | 2 MB | Enforced at Mojo IPC boundary |
| Firefox | 1 MB (standard) | Configurable via prefs |
| WebKit | ~2 GB | Limited by String::MaxLength |

### Lessons Learned
- The 2 KB limit often cited is historical and not enforced by modern browsers
- Different contexts have different limits (IPC, history storage, etc.)
- Firefox is most configurable with multiple preference settings
- WebKit/Safari effectively has no practical URL limit in the parser
