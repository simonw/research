#!/usr/bin/env python3
"""
Extract wheel metadata using HTTP range requests.

This replicates the mechanism used by uv (https://github.com/astral-sh/uv)
to fetch wheel METADATA without downloading the entire wheel file.

ZIP files store their central directory at the end, so we:
1. Fetch the last ~16KB to get the End of Central Directory Record (EOCD)
2. Parse the EOCD to find where the central directory starts
3. Fetch the central directory entries
4. Find the METADATA file entry
5. Fetch just that file's data

This typically requires only 2-3 small HTTP requests instead of downloading
a multi-megabyte wheel file.
"""

import struct
import sys
import zlib
from dataclasses import dataclass
from typing import Optional

import httpx


# ZIP format constants
EOCD_SIGNATURE = b'\x50\x4b\x05\x06'  # End of central directory signature
EOCD_SIZE = 22  # Minimum EOCD size (without comment)
EOCD64_LOCATOR_SIGNATURE = b'\x50\x4b\x06\x07'
EOCD64_SIGNATURE = b'\x50\x4b\x06\x06'
CENTRAL_DIR_SIGNATURE = b'\x50\x4b\x01\x02'  # Central directory file header
LOCAL_FILE_HEADER_SIZE = 30  # Size of local file header (without name/extra)

# How much to fetch initially (same as uv's CENTRAL_DIRECTORY_SIZE)
INITIAL_FETCH_SIZE = 16384


@dataclass
class CentralDirectoryEntry:
    """Represents a file entry in the ZIP central directory."""
    filename: str
    compressed_size: int
    uncompressed_size: int
    compression_method: int
    header_offset: int
    extra_field_length: int
    filename_length: int


def parse_eocd(data: bytes) -> tuple[int, int, int]:
    """
    Parse the End of Central Directory record.

    Returns: (central_dir_offset, central_dir_size, total_entries)
    """
    # Search backwards for EOCD signature
    # EOCD can be at most 22 + 65535 bytes from the end (due to comment)
    pos = data.rfind(EOCD_SIGNATURE)
    if pos == -1:
        raise ValueError("Could not find End of Central Directory record")

    # Parse EOCD structure:
    # 0-3: signature (already verified)
    # 4-5: disk number
    # 6-7: disk with central directory
    # 8-9: entries on this disk
    # 10-11: total entries
    # 12-15: central directory size
    # 16-19: central directory offset
    # 20-21: comment length

    (
        _disk_num,
        _disk_cd,
        _entries_disk,
        total_entries,
        cd_size,
        cd_offset,
        _comment_len,
    ) = struct.unpack_from('<HHHHIIH', data, pos + 4)

    # Check for ZIP64 (values will be 0xFFFF or 0xFFFFFFFF)
    if cd_offset == 0xFFFFFFFF or total_entries == 0xFFFF:
        # Look for ZIP64 End of Central Directory Locator
        locator_pos = data.rfind(EOCD64_LOCATOR_SIGNATURE)
        if locator_pos != -1:
            # Parse locator to find EOCD64 offset
            eocd64_offset = struct.unpack_from('<Q', data, locator_pos + 8)[0]

            # We may need to fetch more data if EOCD64 is not in our buffer
            # For now, assume it's in our buffer (most files are small enough)
            eocd64_pos = data.rfind(EOCD64_SIGNATURE)
            if eocd64_pos != -1:
                # Parse EOCD64:
                # 0-3: signature
                # 4-11: size of EOCD64 record
                # 12-13: version made by
                # 14-15: version needed
                # 16-19: disk number
                # 20-23: disk with CD
                # 24-31: total entries on disk
                # 32-39: total entries
                # 40-47: CD size
                # 48-55: CD offset
                total_entries = struct.unpack_from('<Q', data, eocd64_pos + 32)[0]
                cd_size = struct.unpack_from('<Q', data, eocd64_pos + 40)[0]
                cd_offset = struct.unpack_from('<Q', data, eocd64_pos + 48)[0]

    return cd_offset, cd_size, total_entries


def parse_central_directory(data: bytes) -> list[CentralDirectoryEntry]:
    """
    Parse all entries in the central directory.
    """
    entries = []
    offset = 0

    while offset < len(data):
        # Check for central directory signature
        if data[offset:offset + 4] != CENTRAL_DIR_SIGNATURE:
            break

        # Parse central directory file header:
        # 0-3: signature
        # 4-5: version made by
        # 6-7: version needed
        # 8-9: flags
        # 10-11: compression method
        # 12-13: last mod time
        # 14-15: last mod date
        # 16-19: crc32
        # 20-23: compressed size
        # 24-27: uncompressed size
        # 28-29: filename length
        # 30-31: extra field length
        # 32-33: comment length
        # 34-35: disk number start
        # 36-37: internal attributes
        # 38-41: external attributes
        # 42-45: relative offset of local header

        (
            _version_made,
            _version_needed,
            _flags,
            compression_method,
            _mod_time,
            _mod_date,
            _crc32,
            compressed_size,
            uncompressed_size,
            filename_len,
            extra_len,
            comment_len,
            _disk_start,
            _internal_attr,
            _external_attr,
            header_offset,
        ) = struct.unpack_from('<HHHHHHIIIHHHHHII', data, offset + 4)

        # Handle ZIP64 extended information
        if compressed_size == 0xFFFFFFFF or uncompressed_size == 0xFFFFFFFF or header_offset == 0xFFFFFFFF:
            # Parse ZIP64 extra field
            extra_start = offset + 46 + filename_len
            extra_data = data[extra_start:extra_start + extra_len]
            zip64_offset = 0
            while zip64_offset < len(extra_data) - 4:
                header_id, field_size = struct.unpack_from('<HH', extra_data, zip64_offset)
                if header_id == 0x0001:  # ZIP64 extra field
                    field_data = extra_data[zip64_offset + 4:zip64_offset + 4 + field_size]
                    idx = 0
                    if uncompressed_size == 0xFFFFFFFF:
                        uncompressed_size = struct.unpack_from('<Q', field_data, idx)[0]
                        idx += 8
                    if compressed_size == 0xFFFFFFFF:
                        compressed_size = struct.unpack_from('<Q', field_data, idx)[0]
                        idx += 8
                    if header_offset == 0xFFFFFFFF:
                        header_offset = struct.unpack_from('<Q', field_data, idx)[0]
                    break
                zip64_offset += 4 + field_size

        filename = data[offset + 46:offset + 46 + filename_len].decode('utf-8', errors='replace')

        entries.append(CentralDirectoryEntry(
            filename=filename,
            compressed_size=compressed_size,
            uncompressed_size=uncompressed_size,
            compression_method=compression_method,
            header_offset=header_offset,
            extra_field_length=extra_len,
            filename_length=filename_len,
        ))

        # Move to next entry
        offset += 46 + filename_len + extra_len + comment_len

    return entries


def find_metadata_entry(entries: list[CentralDirectoryEntry], wheel_name: str) -> Optional[CentralDirectoryEntry]:
    """
    Find the METADATA file in the .dist-info directory.

    The .dist-info directory is named: {distribution}-{version}.dist-info/
    """
    # Parse wheel name to get expected dist-info prefix
    # Format: {distribution}-{version}(-{build tag})?-{python tag}-{abi tag}-{platform tag}.whl
    parts = wheel_name.replace('.whl', '').split('-')
    if len(parts) >= 2:
        dist_info_prefix = f"{parts[0]}-{parts[1]}.dist-info/METADATA"
    else:
        dist_info_prefix = None

    for entry in entries:
        # Match the specific dist-info METADATA file
        if entry.filename.endswith('.dist-info/METADATA'):
            if dist_info_prefix is None or entry.filename == dist_info_prefix:
                return entry

    return None


def decompress_data(data: bytes, method: int) -> bytes:
    """Decompress data based on compression method."""
    if method == 0:  # Stored (no compression)
        return data
    elif method == 8:  # Deflated
        return zlib.decompress(data, -zlib.MAX_WBITS)
    else:
        raise ValueError(f"Unsupported compression method: {method}")


def fetch_wheel_metadata(url: str, verbose: bool = False) -> str:
    """
    Fetch wheel METADATA using HTTP range requests.

    This mimics the behavior of uv's wheel_metadata_from_remote_zip function.
    """
    wheel_name = url.split('/')[-1]

    with httpx.Client(follow_redirects=True, timeout=30.0) as client:
        # Step 1: HEAD request to get file size and check range support
        if verbose:
            print(f"[1] HEAD request to get file size...", file=sys.stderr)

        head_response = client.head(url, headers={'Accept-Encoding': 'identity'})
        head_response.raise_for_status()

        file_size = int(head_response.headers.get('Content-Length', 0))
        accept_ranges = head_response.headers.get('Accept-Ranges', '')

        if verbose:
            print(f"    File size: {file_size:,} bytes", file=sys.stderr)
            print(f"    Accept-Ranges: {accept_ranges}", file=sys.stderr)

        if accept_ranges != 'bytes' and 'bytes' not in accept_ranges:
            raise ValueError("Server does not support range requests")

        # Step 2: Fetch the end of the file to get EOCD and central directory
        # Like uv, we fetch the last 16KB
        fetch_start = max(0, file_size - INITIAL_FETCH_SIZE)

        if verbose:
            print(f"[2] Fetching last {file_size - fetch_start:,} bytes (EOCD + central directory)...", file=sys.stderr)

        range_header = f'bytes={fetch_start}-{file_size - 1}'
        response = client.get(url, headers={
            'Range': range_header,
            'Accept-Encoding': 'identity',
        })

        if response.status_code not in (200, 206):
            raise ValueError(f"Range request failed with status {response.status_code}")

        end_data = response.content

        if verbose:
            print(f"    Received {len(end_data):,} bytes", file=sys.stderr)

        # Step 3: Parse EOCD to find central directory location
        cd_offset, cd_size, total_entries = parse_eocd(end_data)

        if verbose:
            print(f"[3] Parsed EOCD:", file=sys.stderr)
            print(f"    Central directory offset: {cd_offset:,}", file=sys.stderr)
            print(f"    Central directory size: {cd_size:,}", file=sys.stderr)
            print(f"    Total entries: {total_entries}", file=sys.stderr)

        # Step 4: Check if we need to fetch more of the central directory
        # Calculate how much of the central directory is in our buffer
        cd_in_buffer_start = max(0, cd_offset - fetch_start)

        if cd_offset < fetch_start:
            # We need to fetch more data to get the complete central directory
            if verbose:
                print(f"[4] Fetching complete central directory...", file=sys.stderr)

            range_header = f'bytes={cd_offset}-{file_size - 1}'
            response = client.get(url, headers={
                'Range': range_header,
                'Accept-Encoding': 'identity',
            })
            response.raise_for_status()
            cd_data = response.content[:cd_size]
        else:
            # Central directory is already in our buffer
            cd_data = end_data[cd_in_buffer_start:cd_in_buffer_start + cd_size]
            if verbose:
                print(f"[4] Central directory already in buffer", file=sys.stderr)

        # Step 5: Parse central directory entries
        entries = parse_central_directory(cd_data)

        if verbose:
            print(f"[5] Parsed {len(entries)} central directory entries", file=sys.stderr)
            for entry in entries[:10]:  # Show first 10
                print(f"    - {entry.filename} ({entry.compressed_size:,} bytes)", file=sys.stderr)
            if len(entries) > 10:
                print(f"    ... and {len(entries) - 10} more", file=sys.stderr)

        # Step 6: Find METADATA file
        metadata_entry = find_metadata_entry(entries, wheel_name)
        if metadata_entry is None:
            raise ValueError("Could not find METADATA file in wheel")

        if verbose:
            print(f"[6] Found METADATA: {metadata_entry.filename}", file=sys.stderr)
            print(f"    Offset: {metadata_entry.header_offset:,}", file=sys.stderr)
            print(f"    Compressed size: {metadata_entry.compressed_size:,}", file=sys.stderr)
            print(f"    Compression method: {metadata_entry.compression_method}", file=sys.stderr)

        # Step 7: Fetch the METADATA file content
        # Calculate how much to fetch: local header + filename + extra + compressed data
        # Local header is 30 bytes, then filename and extra fields, then data
        fetch_size = (
            LOCAL_FILE_HEADER_SIZE +
            metadata_entry.filename_length +
            metadata_entry.extra_field_length +
            metadata_entry.compressed_size +
            1024  # Extra buffer for safety (extra field might differ from central dir)
        )

        if verbose:
            print(f"[7] Fetching METADATA content ({fetch_size:,} bytes)...", file=sys.stderr)

        range_header = f'bytes={metadata_entry.header_offset}-{metadata_entry.header_offset + fetch_size - 1}'
        response = client.get(url, headers={
            'Range': range_header,
            'Accept-Encoding': 'identity',
        })
        response.raise_for_status()

        local_data = response.content

        # Parse local file header to find actual data start
        # Local header structure is similar but not identical to central dir
        (
            _sig,
            _version,
            _flags,
            compression,
            _mod_time,
            _mod_date,
            _crc,
            compressed_size,
            uncompressed_size,
            filename_len,
            extra_len,
        ) = struct.unpack_from('<IHHHHHIIIHH', local_data, 0)

        data_start = LOCAL_FILE_HEADER_SIZE + filename_len + extra_len
        compressed_data = local_data[data_start:data_start + metadata_entry.compressed_size]

        # Decompress if needed
        metadata_content = decompress_data(compressed_data, metadata_entry.compression_method)

        if verbose:
            print(f"[8] Decompressed METADATA: {len(metadata_content):,} bytes", file=sys.stderr)
            total_fetched = len(end_data) + len(local_data)
            savings = (1 - total_fetched / file_size) * 100
            print(f"\nTotal bytes fetched: {total_fetched:,} / {file_size:,} ({savings:.1f}% savings)", file=sys.stderr)

        return metadata_content.decode('utf-8')


def format_metadata(metadata: str) -> str:
    """Format metadata for display."""
    lines = []
    in_description = False

    for line in metadata.split('\n'):
        if in_description:
            lines.append(f"  {line}")
        elif line.startswith('Description:'):
            lines.append(line)
            in_description = True
        elif ': ' in line:
            key, value = line.split(': ', 1)
            lines.append(f"\033[1m{key}\033[0m: {value}")
        else:
            lines.append(line)

    return '\n'.join(lines)


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description='Extract wheel metadata using HTTP range requests',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s https://files.pythonhosted.org/packages/.../package-1.0.0-py3-none-any.whl
  %(prog)s -v https://files.pythonhosted.org/packages/.../package-1.0.0-py3-none-any.whl

This tool uses HTTP range requests to fetch only the parts of a wheel file
needed to extract the METADATA, saving bandwidth and time compared to
downloading the entire wheel.
        """
    )
    parser.add_argument('url', help='URL to the wheel file')
    parser.add_argument('-v', '--verbose', action='store_true',
                        help='Show detailed progress information')
    parser.add_argument('--raw', action='store_true',
                        help='Output raw metadata without formatting')

    args = parser.parse_args()

    try:
        metadata = fetch_wheel_metadata(args.url, verbose=args.verbose)

        if args.verbose:
            print("\n" + "=" * 60, file=sys.stderr)
            print("METADATA:", file=sys.stderr)
            print("=" * 60, file=sys.stderr)

        if args.raw:
            print(metadata)
        else:
            print(format_metadata(metadata))

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
