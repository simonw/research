#!/usr/bin/env python3
"""Build a JSON index for turbolite compressed files.

The turbolite format stores pages sequentially:
  Header (64 bytes): magic "SQLCEvfS", page_size(u32), data_start(u64), dict_size(u32), flags(u32)
  Dictionary (optional): dict_size bytes
  Records: page_num(u64 LE) + data_size(u32 LE) + compressed_data(data_size bytes)

This script scans the file and outputs a JSON index mapping page numbers to
(file_offset, compressed_size) pairs so the browser can do targeted range requests.
"""
import struct
import json
import sys
import os

MAGIC = b"SQLCEvfS"
HEADER_SIZE = 64
RECORD_HEADER_SIZE = 12


def parse_header(data):
    if data[:8] != MAGIC:
        raise ValueError(f"Not a turbolite file (magic: {data[:8]!r})")
    page_size = struct.unpack_from("<I", data, 8)[0]
    data_start = struct.unpack_from("<Q", data, 12)[0]
    dict_size = struct.unpack_from("<I", data, 20)[0]
    flags = struct.unpack_from("<I", data, 24)[0]
    return {
        "pageSize": page_size,
        "dataStart": data_start,
        "dictSize": dict_size,
        "flags": flags,
    }


def build_index(filepath):
    file_size = os.path.getsize(filepath)
    with open(filepath, "rb") as f:
        header_bytes = f.read(HEADER_SIZE)
        header = parse_header(header_bytes)

        # Read dictionary if present
        dict_offset = HEADER_SIZE
        dict_size = header["dictSize"]

        pos = header["dataStart"]
        f.seek(pos)

        pages = {}  # page_num -> {offset, size}
        max_page = 0

        while True:
            rec_hdr = f.read(RECORD_HEADER_SIZE)
            if len(rec_hdr) < RECORD_HEADER_SIZE:
                break

            page_num = struct.unpack_from("<Q", rec_hdr, 0)[0]
            data_size = struct.unpack_from("<I", rec_hdr, 8)[0]

            if data_size > 1_000_000:
                print(f"Warning: suspiciously large record at offset {pos}: page={page_num} size={data_size}")
                break

            data_offset = pos + RECORD_HEADER_SIZE
            pages[str(page_num)] = [data_offset, data_size]
            if page_num > max_page:
                max_page = page_num

            f.seek(data_size, 1)  # skip data
            pos = data_offset + data_size

    index = {
        "format": "turbolite",
        "pageSize": header["pageSize"],
        "pageCount": max_page,
        "fileSize": file_size,
        "dataStart": header["dataStart"],
        "dictSize": header["dictSize"],
        "dictOffset": HEADER_SIZE if dict_size > 0 else None,
        "pages": pages,
    }

    return index


if __name__ == "__main__":
    for filepath in sys.argv[1:]:
        print(f"Indexing {filepath}...")
        index = build_index(filepath)
        out_path = filepath + ".index.json"
        with open(out_path, "w") as f:
            json.dump(index, f)
        print(f"  {len(index['pages'])} pages, index written to {out_path}")
        print(f"  Page size: {index['pageSize']}, file size: {index['fileSize']:,}")
