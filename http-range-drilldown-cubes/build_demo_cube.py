#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy"]
# ///
"""Build nyc311-demo.dcb2: a small real-data cube for the browser demo,
carved out of the full nyc311-cube-v15.dcb1 (130MB, not committed — rebuild it
with parquet_to_dcb1.py first).

Sections included and why:

  all:agency+complaint+borough+channel   4,794 rows   every leaderboard, any
                                                      filter, unbrushed — the
                                                      client fetches it once
  day:overall                            5,006 rows   the headline daily line
  day:agency / day:borough /                          the daily line under a
  day:channel / day:complaint                         single leaderboard click
  week:by-date                         796,645 rows   brushed leaderboards

week:by-date is the full-dimension weekly section from the source cube,
re-sorted with the date FIRST in the sort key: brushes filter on time, so the
brush-serving section puts time first and any brush becomes one contiguous
range read. Dimension filters on it are residual (applied client-side after
decode) — the byte range is the same either way.

Output is DCB2 (columnar deflate-raw blocks, deflated header) readable by
cube-reader2.js.
"""
import json, struct, sys, time, zlib
import numpy as np

SRC = sys.argv[1] if len(sys.argv) > 1 else "nyc311-cube-v15.dcb1"
OUT = sys.argv[2] if len(sys.argv) > 2 else "nyc311-demo.dcb2"
MIN_EVERY = 1024
MAX_INDEX_ENTRIES = 1024
LEVEL = 6

NP_TYPE = {"u8": "<u1", "u16": "<u2", "u32": "<u4", "date-u16": "<u2", "year-u16": "<u2"}
SIZE = {"u8": 1, "u16": 2, "u32": 4, "date-u16": 2, "year-u16": 2}
deflate_raw = lambda b: (lambda c: c.compress(b) + c.flush())(
    zlib.compressobj(LEVEL, zlib.DEFLATED, -15))

t0 = time.time()
src = open(SRC, "rb")
assert src.read(4) == b"DCB1", "input must be a DCB1 file"
hlen = struct.unpack("<I", src.read(4))[0]
v1 = json.loads(src.read(hlen))
data_start = 8 + hlen

def load(name):
    s = v1["sections"][name]
    src.seek(data_start + s["offset"])
    rec = np.frombuffer(src.read(s["rows"] * s["rowSize"]),
                        dtype=np.dtype([(c, NP_TYPE[t]) for c, t in s["columns"]]))
    return s, rec

FULL = "agency+complaint+borough+channel"
plan = []  # (out_name, columns in output order, record array sorted to match)

for name in [f"all:{FULL}", "day:overall", "day:agency", "day:borough",
             "day:channel", "day:complaint"]:
    s, rec = load(name)
    plan.append((name, s["columns"], rec))  # already sorted correctly

# week:by-date — same rows as week:full, date-first sort key.
s, rec = load(f"week:{FULL}")
tmap = dict(s["columns"])
new_cols = [["d", tmap["d"]], ["agency", tmap["agency"]],
            ["complaint", tmap["complaint"]], ["borough", tmap["borough"]],
            ["channel", tmap["channel"]], ["n", tmap["n"]]]
key = [c for c, _ in new_cols[:-1]]
rec = rec[np.lexsort(tuple(rec[c] for c in reversed(key)))]
plan.append(("week:by-date", new_cols, rec))

sections, blobs, offset, raw_total = {}, [], 0, 0
for name, cols, rec in plan:
    names = [c for c, _ in cols]
    key = names[:-1]
    row_size = sum(SIZE[t] for _, t in cols)
    every = MIN_EVERY
    while -(-len(rec) // every) > MAX_INDEX_ENTRIES:
        every *= 2
    keys, offsets, parts, pos = [], [], [], 0
    for b0 in range(0, len(rec), every):
        b1 = min(b0 + every, len(rec))
        keys.append([int(rec[c][b0]) for c in key])
        payload = b"".join(rec[c][b0:b1].tobytes() for c in names)  # columnar
        comp = deflate_raw(payload)
        offsets.append(pos); parts.append(comp); pos += len(comp)
    sections[name] = {
        "offset": offset, "bytes": pos, "rows": int(len(rec)), "rowSize": row_size,
        "codec": "deflate-raw", "layout": "columnar",
        "columns": cols, "sortKey": key,
        "index": {"every": every, "keys": keys, "offsets": offsets},
    }
    blobs.append(b"".join(parts))
    offset += pos
    raw_total += len(rec) * row_size

hjson = json.dumps({"format": "dcb2", "dicts": v1["dicts"], "sections": sections},
                   separators=(",", ":")).encode()
hcomp = deflate_raw(hjson)
with open(OUT, "wb") as f:
    f.write(b"DCB2"); f.write(struct.pack("<I", len(hcomp))); f.write(hcomp)
    for b in blobs:
        f.write(b)

total = 8 + len(hcomp) + offset
print(f"-> {OUT}: {total:,} bytes in {time.time()-t0:.1f}s "
      f"(header {len(hjson):,} -> {len(hcomp):,} B; data {raw_total:,} -> {offset:,} B)\n")
print(f"{'section':<40}{'rows':>9}  {'raw MB':>7}  {'comp MB':>8}  {'every':>6}")
for name, s in sections.items():
    raw = s["rows"] * s["rowSize"]
    print(f"{name:<40}{s['rows']:>9,}  {raw/1e6:>7.2f}  {s['bytes']/1e6:>8.2f}  {s['index']['every']:>6}")
