#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy"]
# ///
"""Recompress a DCB1 file into DCB2 (lossless). Each index-aligned block of
rows is transposed to columnar order and deflate-raw compressed; the sparse
index gains a byte offset per block; the JSON header is itself deflated.

Pass --add-week-by-date to append a second copy of the full-dimension weekly
section sorted with the date first. The GitHub Pages build uses this mode so
demo.html can query the complete cube while retaining efficient date brushes.

DCB2 layout:
  bytes 0-3   magic "DCB2"
  bytes 4-7   uint32 LE: compressed header length H
  bytes 8..   deflate-raw( JSON header )
  8+H ..      compressed blocks, back to back, section by section
"""
import argparse, json, struct, time, zlib
import numpy as np

parser = argparse.ArgumentParser()
parser.add_argument("src", nargs="?", default="nyc311-cube-v15.dcb1")
parser.add_argument("out", nargs="?", default="nyc311-cube-v15.dcb2")
parser.add_argument("--add-week-by-date", action="store_true",
                    help="append a date-first weekly section for demo brushing")
args = parser.parse_args()
SRC, OUT = args.src, args.out
MIN_EVERY = 1024        # blocks below ~1KB compress poorly (measured 1.6x)
MAX_INDEX_ENTRIES = 1024
LEVEL = 6               # measured: level 9 and zstd-12 gain almost nothing
FULL_WEEK = "week:agency+complaint+borough+channel"

NP_TYPE = {"u8": "<u1", "u16": "<u2", "u32": "<u4", "date-u16": "<u2", "year-u16": "<u2"}
deflate_raw = lambda b: (lambda c: c.compress(b) + c.flush())(
    zlib.compressobj(LEVEL, zlib.DEFLATED, -15))

t0 = time.time()
src = open(SRC, "rb")
assert src.read(4) == b"DCB1", "input is not a DCB1 file"
hlen = struct.unpack("<I", src.read(4))[0]
v1 = json.loads(src.read(hlen))
data_start = 8 + hlen

def compress_section(rec, columns, key_cols):
    names = [c for c, _ in columns]
    every = MIN_EVERY
    while -(-len(rec) // every) > MAX_INDEX_ENTRIES:
        every *= 2

    keys, offsets, parts, pos = [], [], [], 0
    for b0 in range(0, len(rec), every):
        b1 = min(b0 + every, len(rec))
        keys.append([int(rec[c][b0]) for c in key_cols])
        payload = b"".join(rec[c][b0:b1].tobytes() for c in names)  # columnar
        comp = deflate_raw(payload)
        offsets.append(pos); parts.append(comp); pos += len(comp)

    return {
        "bytes": pos, "rows": int(len(rec)),
        "rowSize": rec.dtype.itemsize, "codec": "deflate-raw", "layout": "columnar",
        "columns": columns, "sortKey": key_cols,
        "index": {"every": every, "keys": keys, "offsets": offsets},
    }, b"".join(parts)

sections, blobs, offset, raw_total = {}, [], 0, 0

def append_section(name, rec, columns, key_cols, meta=None):
    global offset, raw_total
    packed, body = compress_section(rec, columns, key_cols)
    # Preserve the original header key order so default-mode output remains
    # byte-for-byte reproducible with the initial DCB2 conversion.
    section = {
        "offset": offset, "bytes": packed["bytes"], "rows": packed["rows"],
        "rowSize": packed["rowSize"], "codec": packed["codec"],
        "layout": packed["layout"], "columns": packed["columns"],
        "sortKey": packed["sortKey"], "index": packed["index"],
        "meta": meta or {},
    }
    sections[name] = section
    blobs.append(body)
    offset += len(body)
    raw_total += len(rec) * rec.dtype.itemsize

for name, s1 in v1["sections"].items():
    src.seek(data_start + s1["offset"])
    rec = np.frombuffer(src.read(s1["rows"] * s1["rowSize"]),
                        dtype=np.dtype([(c, NP_TYPE[t]) for c, t in s1["columns"]]))
    append_section(name, rec, s1["columns"], s1["sortKey"], s1.get("meta"))

    if args.add_week_by_date and name == FULL_WEEK:
        types = dict(s1["columns"])
        key_cols = ["d", "agency", "complaint", "borough", "channel"]
        columns = [[c, types[c]] for c in key_cols] + [["n", types["n"]]]
        order = np.lexsort(tuple(rec[c] for c in reversed(key_cols)))
        date_first = rec[order]
        meta = {**s1.get("meta", {}), "derivedFrom": FULL_WEEK}
        append_section("week:by-date", date_first, columns, key_cols, meta)

if args.add_week_by_date and "week:by-date" not in sections:
    raise ValueError(f"--add-week-by-date requires a {FULL_WEEK!r} section")

hjson = json.dumps({"format": "dcb2", "dicts": v1["dicts"], "sections": sections},
                   separators=(",", ":")).encode()
hcomp = deflate_raw(hjson)
with open(OUT, "wb") as f:
    f.write(b"DCB2"); f.write(struct.pack("<I", len(hcomp))); f.write(hcomp)
    for b in blobs:
        f.write(b)

total = 8 + len(hcomp) + offset
print(f"{SRC} -> {OUT}: {total:,} bytes in {time.time()-t0:.1f}s")
print(f"header {len(hjson):,} -> {len(hcomp):,} B   data {raw_total:,} -> {offset:,} B "
      f"({raw_total/offset:.2f}x)\n")
print(f"{'section':<42}{'rows':>10}  {'raw MB':>7}  {'comp MB':>8}  {'ratio':>6}  {'every':>6}")
for name, s in sections.items():
    raw = s["rows"] * s["rowSize"]
    print(f"{name:<42}{s['rows']:>10,}  {raw/1e6:>7.1f}  {s['bytes']/1e6:>8.2f}  "
          f"{raw/s['bytes']:>5.1f}x  {s['index']['every']:>6}")
