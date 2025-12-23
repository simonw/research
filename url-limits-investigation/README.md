# Browser URL Size Limits Investigation

This investigation examines the maximum URL size limits enforced by the three major browser engines: Chromium, Firefox (Gecko), and WebKit.

## Summary

| Browser Engine | Maximum URL Length | Where Enforced |
|----------------|-------------------|----------------|
| **Chromium** | 2 MB (2,097,152 bytes) | Mojo IPC boundary |
| **Firefox** | 1 MB (standard) / 512 MB (absolute) | Configurable via preferences |
| **WebKit** | ~2 GB | String::MaxLength (INT32_MAX) |

## Chromium

### Limit: 2 MB (2,097,152 characters)

Chromium enforces a 2 MB limit on URLs passed between processes via Mojo IPC. URLs longer than this are silently replaced with empty, invalid URLs.

**Source File:** `url/url_constants.h` ([GitHub permalink](https://github.com/chromium/chromium/blob/eae506cc8e9b1cd874a63d20d4d006a1428d29ec/url/url_constants.h#L68-L70))

```cpp
// Max GURL length passed between processes. See url::mojom::kMaxURLChars, which
// has the same value, for more details.
inline constexpr size_t kMaxURLChars = 2 * 1024 * 1024;
```

**Mojo Interface:** `url/mojom/url.mojom` ([GitHub permalink](https://github.com/chromium/chromium/blob/eae506cc8e9b1cd874a63d20d4d006a1428d29ec/url/mojom/url.mojom#L7-L15))

```cpp
// The longest GURL length that may be passed over Mojo pipes. Longer GURLs
// may be created and will be considered valid, but when pass over Mojo, URLs
// longer than this are silently replaced with empty, invalid GURLs. Anything
// receiving GURLs must be prepared to receive invalid GURLs without reporting
// a bad message, unless there's a length check before sending the GURL over a
// Mojo pipe.
//
// 2 * 1024 * 1024
const uint32 kMaxURLChars = 2097152;
```

**Enforcement Location:** `url/mojom/url_gurl_mojom_traits.cc` ([GitHub permalink](https://github.com/chromium/chromium/blob/eae506cc8e9b1cd874a63d20d4d006a1428d29ec/url/mojom/url_gurl_mojom_traits.cc#L14))

```cpp
if (r.possibly_invalid_spec().length() > url::kMaxURLChars || !r.is_valid()) {
```

## Firefox (Gecko)

### Multiple Configurable Limits

Firefox uses configurable preferences for URL length limits, allowing different limits for different contexts.

**Source File:** `modules/libpref/init/StaticPrefList.yaml`

#### 1. Standard URL Maximum (1 MB)

([GitHub permalink](https://github.com/mozilla-firefox/firefox/blob/20a1fb35a4d5c2f2ea6c865ecebc8e4bee6f86c9/modules/libpref/init/StaticPrefList.yaml#L14761-L14765))

```yaml
# The maximum allowed length for a URL - 1MB default.
- name: network.standard-url.max-length
  type: RelaxedAtomicUint32
  value: 1048576
  mirror: always
```

#### 2. Absolute URL Maximum (512 MB)

([GitHub permalink](https://github.com/mozilla-firefox/firefox/blob/20a1fb35a4d5c2f2ea6c865ecebc8e4bee6f86c9/modules/libpref/init/StaticPrefList.yaml#L15114-L15119))

```yaml
# The maximum allowed length for a URL - 512MB default.
# If 0 that means no limit.
- name: network.url.max-length
  type: RelaxedAtomicUint32
  value: 512 * 1024 * 1024
  mirror: always
```

#### 3. History/Bookmarks URL Maximum (2,000 characters)

([GitHub permalink](https://github.com/mozilla-firefox/firefox/blob/20a1fb35a4d5c2f2ea6c865ecebc8e4bee6f86c9/modules/libpref/init/StaticPrefList.yaml#L1680-L1689))

```yaml
# * Sitemaps protocol used to support a maximum of 2048 chars
# * Various SEO guides suggest to not go over 2000 chars
# * Various apps/services are known to have issues over 2000 chars
# * RFC 2616 - HTTP/1.1 suggests being cautious about depending
#   on URI lengths above 255 bytes
#
- name: browser.history.maxUrlLength
  type: uint32_t
  value: 2000
  mirror: always
```

## WebKit (Safari)

### Limit: ~2 GB (INT32_MAX = 2,147,483,647 bytes)

WebKit does not enforce a URL-specific length limit. URLs are stored as `WTF::String` objects, which have a maximum length defined by the string implementation.

**Source File:** `Source/WTF/wtf/text/StringImpl.h` ([GitHub permalink](https://github.com/WebKit/WebKit/blob/0a662721c2e04557824adff2324915aa0b5c8341/Source/WTF/wtf/text/StringImpl.h#L152))

```cpp
static constexpr unsigned MaxLength = std::numeric_limits<int32_t>::max();
```

This is then referenced by the String class:

**Source File:** `Source/WTF/wtf/text/WTFString.h` ([GitHub permalink](https://github.com/WebKit/WebKit/blob/0a662721c2e04557824adff2324915aa0b5c8341/Source/WTF/wtf/text/WTFString.h#L315))

```cpp
static constexpr unsigned MaxLength = StringImpl::MaxLength;
```

The URL class (`Source/WTF/wtf/URL.h`) uses String internally without additional length restrictions on the URL itself.

## Key Observations

1. **No standard limit exists**: Each browser engine implements its own limits independently.

2. **Context matters**: Different limits apply in different contexts:
   - IPC/process boundaries (Chromium's 2MB limit)
   - URL parsing (Firefox's 1MB standard limit)
   - History storage (Firefox's 2,000 character limit for history)

3. **Historical limits are outdated**: The commonly cited "2KB URL limit" is not enforced by any modern browser engine examined.

4. **Practical limits**: While WebKit theoretically allows ~2GB URLs, practical limits are imposed by:
   - Server configurations
   - Network infrastructure
   - Memory constraints
   - Other browser components (not examined in this investigation)

## Methodology

- Cloned source repositories from GitHub (shallow clone, depth=1)
- Searched for patterns: `kMaxURL`, `max.*url`, `URL.*limit`, `MaxLength`
- Examined URL parsing and IPC serialization code
- Identified configuration preferences

## Repository Commits Analyzed

- **Chromium**: `eae506cc8e9b1cd874a63d20d4d006a1428d29ec`
- **Firefox**: `20a1fb35a4d5c2f2ea6c865ecebc8e4bee6f86c9`
- **WebKit**: `0a662721c2e04557824adff2324915aa0b5c8341`

Date of analysis: December 23, 2025
