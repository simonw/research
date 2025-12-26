# HTTP Range Requests for Wheel Metadata

An investigation into how [uv](https://github.com/astral-sh/uv) extracts wheel metadata using HTTP range requests, plus a Python recreation.

## Overview

When resolving Python package dependencies, tools like `pip` and `uv` need to read the `METADATA` file from wheel archives. Traditional approaches download the entire wheel, but wheels can be megabytes in size while the METADATA file is typically just a few kilobytes.

**uv's approach:**
1. Try PEP 658 metadata (`.metadata` file provided by package index) - fastest
2. Use HTTP range requests to read only the central directory and METADATA from the zip - fast
3. Stream the full wheel - slow fallback
4. Build from source distribution - slowest

This investigation focuses on step 2: the HTTP range request mechanism.

## How ZIP Files Work

ZIP files are unique in that their file listing (central directory) is stored at the **end** of the archive:

```
┌─────────────────────────────────────────┐
│  Local File Header 1                    │
│  File Data 1 (compressed)               │
│  Local File Header 2                    │
│  File Data 2 (compressed)               │
│  ...                                    │
├─────────────────────────────────────────┤
│  Central Directory Entry 1              │  ← Contains offsets to each file
│  Central Directory Entry 2              │
│  ...                                    │
├─────────────────────────────────────────┤
│  End of Central Directory Record (EOCD) │  ← Points to central directory
└─────────────────────────────────────────┘
```

This structure allows us to:
1. Fetch the last ~16KB to get the EOCD and central directory
2. Parse the central directory to find the METADATA file's offset
3. Fetch just that file's content

## uv's Implementation (Rust)

### Key Files

The core implementation is in `crates/uv-client/src/remote_metadata.rs`:

```rust
/// Read the `.dist-info/METADATA` file from a async remote zip reader
pub(crate) async fn wheel_metadata_from_remote_zip(
    filename: &WheelFilename,
    debug_name: &Url,
    reader: &mut AsyncHttpRangeReader,
) -> Result<String, Error> {
    // Best guess for the central directory size inside the zip
    const CENTRAL_DIRECTORY_SIZE: u64 = 16384;

    // Prefetch the back part of the stream (where the central directory lives)
    reader
        .prefetch(reader.len().saturating_sub(CENTRAL_DIRECTORY_SIZE)..reader.len())
        .await;

    // Construct a zip reader to parse the structure
    let buf = BufReader::new(reader.compat());
    let mut reader = async_zip::base::read::seek::ZipFileReader::new(buf)
        .await
        .map_err(|err| ErrorKind::Zip(filename.clone(), err))?;

    // Find the METADATA file in the .dist-info directory
    let ((metadata_idx, metadata_entry), _dist_info_prefix) = find_archive_dist_info(
        filename,
        reader.file().entries().iter().enumerate()
            .filter_map(|(idx, e)| Some(((idx, e), e.filename().as_str().ok()?))),
    )?;

    // Calculate exact byte range needed
    let offset = metadata_entry.header_offset();
    let size = metadata_entry.compressed_size()
        + 30  // Local file header size
        + metadata_entry.filename().as_bytes().len() as u64;

    // Prefetch just the METADATA file content
    reader.inner_mut().get_mut().get_mut()
        .prefetch(offset..offset + size)
        .await;

    // Read and decompress METADATA
    let mut contents = String::new();
    reader.reader_with_entry(metadata_idx).await?
        .read_to_string_checked(&mut contents).await?;

    Ok(contents)
}
```

The fallback chain in `registry_client.rs`:

```rust
async fn wheel_metadata_no_pep658<'data>(
    &self,
    filename: &'data WheelFilename,
    url: &'data DisplaySafeUrl,
    // ...
) -> Result<ResolutionMetadata, Error> {
    // First try: HTTP range requests
    if capabilities.supports_range_requests(index) {
        let req = self.uncached_client(url).head(Url::from(url.clone()))
            .header("accept-encoding", "identity")
            .build()?;

        let read_metadata_range_request = |response: Response| {
            async {
                let mut reader = AsyncHttpRangeReader::from_head_response(
                    self.uncached_client(url).clone(),
                    response,
                    url.clone(),
                    headers.clone(),
                ).await?;

                wheel_metadata_from_remote_zip(filename, url, &mut reader).await
            }
        };

        match self.cached_client().get_serde_with_retry(req, ...).await {
            Ok(metadata) => return Ok(metadata),
            Err(err) if err.is_http_range_requests_unsupported() => {
                warn!("Range requests not supported; streaming wheel");
                // Fall back to streaming the full wheel
            }
            Err(err) => return Err(err),
        }
    }

    // Fallback: stream the full wheel
    // ...
}
```

## Python Recreation

I've created `wheel_metadata.py` which replicates this mechanism using `httpx`:

```python
def fetch_wheel_metadata(url: str, verbose: bool = False) -> str:
    """Fetch wheel METADATA using HTTP range requests."""

    with httpx.Client(follow_redirects=True, timeout=30.0) as client:
        # Step 1: HEAD request to get file size and check range support
        head_response = client.head(url, headers={'Accept-Encoding': 'identity'})
        file_size = int(head_response.headers.get('Content-Length', 0))

        # Step 2: Fetch the last 16KB (EOCD + central directory)
        fetch_start = max(0, file_size - 16384)
        range_header = f'bytes={fetch_start}-{file_size - 1}'
        response = client.get(url, headers={'Range': range_header, ...})
        end_data = response.content

        # Step 3: Parse EOCD to find central directory
        cd_offset, cd_size, total_entries = parse_eocd(end_data)

        # Step 4: Parse central directory to find METADATA
        entries = parse_central_directory(cd_data)
        metadata_entry = find_metadata_entry(entries, wheel_name)

        # Step 5: Fetch just the METADATA file content
        range_header = f'bytes={metadata_entry.header_offset}-...'
        response = client.get(url, headers={'Range': range_header, ...})

        # Step 6: Decompress and return
        return decompress_data(compressed_data, method).decode('utf-8')
```

### Testing

```bash
$ python wheel_metadata.py -v "https://files.pythonhosted.org/packages/.../llm-0.28-py3-none-any.whl"

[1] HEAD request to get file size...
    File size: 82,559 bytes
    Accept-Ranges: bytes
[2] Fetching last 16,384 bytes (EOCD + central directory)...
    Received 16,384 bytes
[3] Parsed EOCD:
    Central directory offset: 80,988
    Central directory size: 1,549
    Total entries: 23
[4] Central directory already in buffer
[5] Parsed 23 central directory entries
[6] Found METADATA: llm-0.28.dist-info/METADATA
    Offset: 72,807
    Compressed size: 6,711
[7] Fetching METADATA content (7,792 bytes)...
[8] Decompressed METADATA: 29,657 bytes

Total bytes fetched: 24,176 / 82,559 (70.7% savings)
```

**70.7% bandwidth savings** by fetching only 24KB instead of the full 82KB wheel!

---

# Bonus: Compact Version Representation

uv also optimizes version comparison by packing PEP 440 versions into a single `u64`:

## The Problem

Package managers compare millions of versions during dependency resolution. String parsing and comparison for each version is slow.

## uv's Solution

Pack version information into 64 bits:

```
┌─────────────────────────────────────────────────────────────────┐
│ Bits 63-48   47-40     39-32     31-24     23-20     19-0       │
│ Release[0]  Release[1] Release[2] Release[3] Kind    Version    │
│ (16 bits)   (8 bits)   (8 bits)   (8 bits)  (4 bits) (20 bits)  │
└─────────────────────────────────────────────────────────────────┘
```

From `crates/uv-pep440/src/version.rs`:

```rust
struct VersionSmall {
    len: u8,    // Number of release segments (0-4)
    repr: u64,  // Packed representation
    _force_niche: NonZero<u8>,  // Optimization for enum size
}

impl VersionSmall {
    // Suffix kinds form an enumeration that provides correct ordering
    const SUFFIX_MIN: u64 = 0;       // For lower bounds
    const SUFFIX_DEV: u64 = 1;       // 1.0.dev0
    const SUFFIX_PRE_ALPHA: u64 = 2; // 1.0a0
    const SUFFIX_PRE_BETA: u64 = 3;  // 1.0b0
    const SUFFIX_PRE_RC: u64 = 4;    // 1.0rc0
    const SUFFIX_NONE: u64 = 5;      // 1.0 (final release)
    const SUFFIX_LOCAL: u64 = 6;     // 1.0+local
    const SUFFIX_POST: u64 = 7;      // 1.0.post1
    const SUFFIX_MAX: u64 = 8;       // For upper bounds

    fn push_release(&mut self, n: u64) -> bool {
        if self.len == 0 {
            // First segment: 16 bits (max 65535)
            if n > u64::from(u16::MAX) { return false; }
            self.repr |= n << 48;
            self.len = 1;
            true
        } else {
            // Other segments: 8 bits (max 255)
            if n > u64::from(u8::MAX) { return false; }
            if self.len >= 4 { return false; }
            let shift = 48 - (usize::from(self.len) * 8);
            self.repr |= n << shift;
            self.len += 1;
            true
        }
    }
}
```

### The Key Insight

The bit layout is designed so that **integer comparison equals version comparison**:

```rust
impl Ord for VersionSmall {
    fn cmp(&self, other: &Self) -> Ordering {
        self.repr.cmp(&other.repr)  // Single integer comparison!
    }
}
```

This works because:
1. Release segments are in the high bits (most significant)
2. Suffix kind values are ordered: DEV < ALPHA < BETA < RC < NONE < POST
3. Suffix version is in the low bits

## Python Recreation

`version_packing.py` demonstrates this:

```python
class VersionSmall:
    """Compact version packed into u64."""

    def __lt__(self, other: 'VersionSmall') -> bool:
        # The magic: version comparison is just integer comparison!
        return self.repr < other.repr
```

```bash
$ python version_packing.py --demo

Version Comparison Demo
============================================================

Original order:
  1.0.0 (repr=0x0001000000500000)
  1.0.0a1 (repr=0x0001000000200001)
  1.0.0b1 (repr=0x0001000000300001)
  1.0.0rc1 (repr=0x0001000000400001)
  1.0.0.post1 (repr=0x0001000000700001)
  1.0.1 (repr=0x0001000100500000)
  2.0.0.dev1 (repr=0x0002000000100001)
  2.0.0 (repr=0x0002000000500000)

Sorted order (by integer comparison of packed u64):
  1.0.0a1 (repr=0x0001000000200001)
  1.0.0b1 (repr=0x0001000000300001)
  1.0.0rc1 (repr=0x0001000000400001)
  1.0.0 (repr=0x0001000000500000)
  1.0.0.post1 (repr=0x0001000000700001)
  1.0.1 (repr=0x0001000100500000)
  2.0.0.dev1 (repr=0x0002000000100001)
  2.0.0 (repr=0x0002000000500000)
```

Note how the hex values increase monotonically in correct PEP 440 order!

### Coverage Statistics

From uv's analysis of PyPI:

```
JUST release counts: 7621098 (67.66%)
dev-releases: 765099 (6.79%)
locals: 1 (0.00%)
fitsu8: 10388430 (92.23%)      <- All segments fit in u8/u16
sweetspot: 10236089 (90.87%)   <- Fits in compact repr
```

**Over 90% of PyPI versions fit in the compact u64 representation!**

---

## Files Included

- `wheel_metadata.py` - Python CLI for HTTP range-based metadata extraction
- `version_packing.py` - Python demonstration of compact version packing
- `notes.md` - Investigation notes
- `README.md` - This report

## Key Takeaways

1. **ZIP structure enables partial downloads**: The central directory at the end means we can fetch metadata without the full file.

2. **HTTP Range requests are widely supported**: PyPI and most CDNs support `Range:` headers.

3. **Bandwidth savings are significant**: 70%+ reduction for typical wheels.

4. **Smart data structures matter**: Packing versions into integers enables O(1) comparison vs O(n) string comparison.

5. **These optimizations compound**: When resolving dependencies across thousands of packages with millions of versions, every microsecond and byte matters.

## References

- [ZIP file format specification](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)
- [PEP 658 – Serve Distribution Metadata in the Simple Repository API](https://peps.python.org/pep-0658/)
- [PEP 440 – Version Identification and Dependency Specification](https://peps.python.org/pep-0440/)
- [uv source code](https://github.com/astral-sh/uv)
- [async-http-range-reader crate](https://docs.rs/async-http-range-reader)
