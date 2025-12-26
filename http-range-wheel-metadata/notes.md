# Investigation Notes: HTTP Range Requests for Wheel Metadata

## Fri Dec 26 23:04:13 UTC 2025

Starting investigation into uv's HTTP range request mechanism for wheel metadata extraction.

## Key Findings from uv Source

### File: `crates/uv-client/src/remote_metadata.rs`
- Uses `async_http_range_reader::AsyncHttpRangeReader` to make partial HTTP requests
- Prefetches last 16KB (CENTRAL_DIRECTORY_SIZE) to get the zip central directory
- Uses `async_zip` crate to parse the zip structure
- Finds the METADATA file in `.dist-info/` directory
- Calculates exact offset and size of the METADATA entry
- Prefetches just that range (offset + header size + compressed size)
- Reads and decompresses the METADATA file

### File: `crates/uv-client/src/registry_client.rs`
- `wheel_metadata_no_pep658` function handles the range request fallback
- First sends a HEAD request to get file size
- Creates `AsyncHttpRangeReader::from_head_response` 
- If range requests not supported, falls back to streaming the full wheel

### The fallback chain:
1. PEP 658 metadata (`.metadata` file provided by index)
2. HTTP range requests for zip central directory → METADATA file
3. Stream full wheel download

## ZIP File Structure

The key insight is that ZIP files store their central directory at the END of the file:
- End of Central Directory Record (EOCD) is at the very end
- EOCD points to the Central Directory
- Central Directory contains offsets to each file

This means we can:
1. Fetch just the last 16KB to get EOCD + Central Directory
2. Parse to find METADATA file offset
3. Fetch just that file's bytes

## Python Recreation

Created `wheel_metadata.py` using httpx that replicates the mechanism:
- HEAD request to get file size and check range support
- Range request for last 16KB
- Parse EOCD and Central Directory
- Range request for just the METADATA file
- Decompress and return

Tested on llm-0.28 wheel:
- Total wheel size: 82,559 bytes
- Total fetched: 24,176 bytes
- Savings: 70.7%

## Version Packing (Bonus Investigation)

Found in `crates/uv-pep440/src/version.rs`:
- Packs versions into u64 for fast comparison
- Release segments in high bits (16+8+8+8 = 40 bits)
- Suffix kind in 4 bits (ordered: DEV < ALPHA < BETA < RC < NONE < POST)
- Suffix version in 20 bits
- Result: Version comparison is single integer comparison

Created `version_packing.py` demonstrating this technique.

Statistics from uv's PyPI analysis:
- 92.23% of versions fit in u8 segments
- 90.87% fit in the compact u64 representation

## Final Summary

Both mechanisms show how careful engineering enables massive performance gains:
1. HTTP range requests: 70%+ bandwidth savings by understanding ZIP structure
2. Version packing: O(1) comparison by packing into integers with careful bit layout

These optimizations compound when resolving dependencies across thousands of packages.
