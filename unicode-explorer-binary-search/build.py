#!/usr/bin/env python3
"""
Build script for Unicode Explorer.
Downloads UnicodeData.txt and Blocks.txt, produces:
  - data/unicode-data.bin  (fixed-width records, 256 bytes each)
  - data/meta.json         (metadata + signposts)
"""

import json
import os
import urllib.request

DATA_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "data")
UNICODE_DATA_URL = "https://www.unicode.org/Public/UCD/latest/ucd/UnicodeData.txt"
BLOCKS_URL = "https://www.unicode.org/Public/UCD/latest/ucd/Blocks.txt"
RECORD_WIDTH = 256


def download(url, filename):
    path = os.path.join(DATA_DIR, filename)
    if os.path.exists(path):
        print(f"  {filename} already exists, skipping download")
        return path
    print(f"  Downloading {filename}...")
    urllib.request.urlretrieve(url, path)
    print(f"  Saved {filename} ({os.path.getsize(path)} bytes)")
    return path


def parse_blocks(path):
    """Parse Blocks.txt into a list of (start, end, name) tuples."""
    blocks = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            # Format: 0000..007F; Basic Latin
            parts = line.split(";")
            if len(parts) != 2:
                continue
            range_part = parts[0].strip()
            name = parts[1].strip()
            start_s, end_s = range_part.split("..")
            blocks.append((int(start_s, 16), int(end_s, 16), name))
    blocks.sort(key=lambda b: b[0])
    return blocks


def find_block(cp, blocks):
    """Find the block name for a codepoint using binary search."""
    lo, hi = 0, len(blocks) - 1
    while lo <= hi:
        mid = (lo + hi) // 2
        start, end, name = blocks[mid]
        if cp < start:
            hi = mid - 1
        elif cp > end:
            lo = mid + 1
        else:
            return name
    return ""


def build():
    os.makedirs(DATA_DIR, exist_ok=True)

    print("Step 1: Download source data")
    unicode_data_path = download(UNICODE_DATA_URL, "UnicodeData.txt")
    blocks_path = download(BLOCKS_URL, "Blocks.txt")

    print("Step 2: Parse blocks")
    blocks = parse_blocks(blocks_path)
    print(f"  Found {len(blocks)} blocks")

    print("Step 3: Parse UnicodeData.txt and build records")
    records = []
    overflow_count = 0

    with open(unicode_data_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            fields = line.split(";")
            cp_hex = fields[0]
            name = fields[1]
            category = fields[2]

            # Handle ranges like "<CJK Ideograph Extension A, First>"
            if name.startswith("<") and name.endswith(">"):
                if ", First" in name or ", Last" in name:
                    # Handled separately below
                    continue
                # For <control> characters, use the Unicode 1.0 name (field 10)
                if name == "<control>":
                    old_name = fields[10] if len(fields) > 10 else ""
                    if old_name:
                        name = old_name
                    else:
                        name = f"CONTROL-{cp_hex}"
                else:
                    # Skip other unnamed entries
                    continue

            cp = int(cp_hex, 16)
            block = find_block(cp, blocks)

            record = {
                "cp": cp,
                "name": name,
                "cat": category,
                "block": block,
            }

            json_str = json.dumps(record, ensure_ascii=False, separators=(",", ":"))
            encoded = json_str.encode("utf-8")

            if len(encoded) > RECORD_WIDTH:
                # Truncate name to fit
                max_name_len = len(name)
                while len(encoded) > RECORD_WIDTH and max_name_len > 10:
                    max_name_len -= 10
                    record["name"] = name[:max_name_len] + "..."
                    json_str = json.dumps(record, ensure_ascii=False, separators=(",", ":"))
                    encoded = json_str.encode("utf-8")

                if len(encoded) > RECORD_WIDTH:
                    overflow_count += 1
                    print(f"  WARNING: Record for U+{cp_hex} still overflows at {len(encoded)} bytes after truncation")
                    continue

            records.append((cp, json_str))

    # Also add CJK unified ideographs and other algorithmically-named ranges
    print("Step 4: Add CJK and Hangul ranges")
    cjk_ranges = []
    with open(unicode_data_path, "r", encoding="utf-8") as f:
        first_cp = None
        first_name = None
        for line in f:
            line = line.strip()
            if not line:
                continue
            fields = line.split(";")
            cp_hex = fields[0]
            name = fields[1]
            category = fields[2]

            if name.startswith("<") and ", First>" in name:
                first_cp = int(cp_hex, 16)
                first_name = name.replace(", First>", "").replace("<", "")
                first_cat = category
            elif name.startswith("<") and ", Last>" in name:
                last_cp = int(cp_hex, 16)
                cjk_ranges.append((first_cp, last_cp, first_name, first_cat))

    for range_start, range_end, range_name, cat in cjk_ranges:
        for cp in range(range_start, range_end + 1):
            block = find_block(cp, blocks)
            # Generate the algorithmic name
            if "CJK" in range_name or "Ideograph" in range_name:
                name = f"CJK UNIFIED IDEOGRAPH-{cp:04X}"
            elif "Hangul Syllable" in range_name:
                name = f"HANGUL SYLLABLE {cp:04X}"
            elif "Tangut" in range_name:
                name = f"TANGUT IDEOGRAPH-{cp:04X}"
            else:
                name = f"{range_name}-{cp:04X}"

            record = {
                "cp": cp,
                "name": name,
                "cat": cat,
                "block": block,
            }

            json_str = json.dumps(record, ensure_ascii=False, separators=(",", ":"))
            encoded = json_str.encode("utf-8")

            if len(encoded) > RECORD_WIDTH:
                record["name"] = name[:40] + "..."
                json_str = json.dumps(record, ensure_ascii=False, separators=(",", ":"))
                encoded = json_str.encode("utf-8")
                if len(encoded) > RECORD_WIDTH:
                    overflow_count += 1
                    continue

            records.append((cp, json_str))

    # Sort by codepoint
    records.sort(key=lambda r: r[0])

    # Remove duplicates (same codepoint)
    seen = set()
    unique_records = []
    for cp, json_str in records:
        if cp not in seen:
            seen.add(cp)
            unique_records.append((cp, json_str))
    records = unique_records

    total_records = len(records)
    print(f"  Total records: {total_records}")
    if overflow_count:
        print(f"  WARNING: {overflow_count} records overflowed and were skipped")

    print("Step 5: Write binary file")
    bin_path = os.path.join(DATA_DIR, "unicode-data.bin")
    with open(bin_path, "wb") as f:
        for cp, json_str in records:
            encoded = json_str.encode("utf-8")
            padded = encoded + b" " * (RECORD_WIDTH - len(encoded))
            assert len(padded) == RECORD_WIDTH, f"Record for cp={cp} is {len(padded)} bytes"
            f.write(padded)

    total_bytes = os.path.getsize(bin_path)
    print(f"  Written {total_bytes} bytes ({total_bytes / 1024 / 1024:.1f} MB)")

    print("Step 6: Compute signposts")
    signposts = []
    for i in range(8):
        idx = (i * total_records) // 8
        if idx >= total_records:
            idx = total_records - 1
        cp = records[idx][0]
        signposts.append({"idx": idx, "cp": cp})

    print("  Signposts:")
    for sp in signposts:
        print(f"    idx={sp['idx']} cp=U+{sp['cp']:04X} ({sp['cp']})")

    print("Step 7: Write meta.json")
    meta = {
        "recordWidth": RECORD_WIDTH,
        "totalRecords": total_records,
        "totalBytes": total_bytes,
        "signposts": signposts,
    }
    meta_path = os.path.join(DATA_DIR, "meta.json")
    with open(meta_path, "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=2)

    print(f"\nDone! {total_records} records, {total_bytes} bytes")
    print(f"  {bin_path}")
    print(f"  {meta_path}")


if __name__ == "__main__":
    build()
