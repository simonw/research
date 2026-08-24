#!/usr/bin/env python3
"""Pack a SQLite `cube` table into a DCB1 file: a single binary designed to be
queried with plain HTTP range requests.

Layout:
  bytes 0-3   magic "DCB1"
  bytes 4-7   uint32 LE: header length H
  bytes 8..   header: one JSON object (dictionaries, section table, sparse index)
  then        section bodies, back to back: fixed-width dictionary-encoded rows,
              sorted by each section's sort key

Section offsets in the header are relative to the end of the header, so the
header's own length never has to be known while building it.
"""
import json, sqlite3, struct, sys
from datetime import date

DB, OUT = (sys.argv + ["cube_demo.db", "cube.bin"])[1:3]
EVERY = 256  # sparse index: record the sort key of every 256th row

DIM_ORDER = ["agency", "complaint_type", "submission_type", "borough"]
EPOCH = date(1970, 1, 1)

def days(iso):
    y, m, d = map(int, iso.split("-"))
    return (date(y, m, d) - EPOCH).days

def int_type(cardinality):
    if cardinality <= 256:    return ("u8", "B")
    if cardinality <= 65536:  return ("u16", "H")
    return ("u32", "I")

rows = sqlite3.connect(DB).execute(
    "SELECT gset, agency, complaint_type, submission_type, borough, period, n FROM cube"
).fetchall()

# Global dictionaries, sorted so that id order == string order. That means the
# client can binary-search on encoded ids and get value-ordered semantics free.
dicts = {c: sorted({r[1 + i] for r in rows if r[1 + i] != "*"})
         for i, c in enumerate(DIM_ORDER)}
lookup = {c: {v: i for i, v in enumerate(vs)} for c, vs in dicts.items()}

by_gset = {}
for r in rows:
    by_gset.setdefault(r[0], []).append(r)

sections, blobs, offset = {}, [], 0
for gset in sorted(by_gset):
    rs = by_gset[gset]
    # A rolled-up dimension is simply absent from this section's schema --
    # the '*' sentinel is a relational artifact the binary format doesn't need.
    active = [c for i, c in enumerate(DIM_ORDER) if any(r[1 + i] != "*" for r in rs)]
    has_period = any(r[5] != "*" for r in rs)
    ptype = None
    if has_period:
        lens = {len(r[5]) for r in rs}
        assert lens <= {4} or lens <= {10}, f"mixed period formats in {gset}"
        ptype = "year-u16" if lens == {4} else "date-u16"

    cols, fmt = [], "<"
    for c in active:
        t, f = int_type(len(dicts[c]))
        cols.append([c, t]); fmt += f
    if has_period:
        cols.append(["period", ptype]); fmt += "H"
    cols.append(["n", "u32"]); fmt += "I"
    row_size = struct.calcsize(fmt)

    enc = []
    for r in rs:
        key = [lookup[c][r[1 + DIM_ORDER.index(c)]] for c in active]
        if has_period:
            key.append(int(r[5]) if ptype == "year-u16" else days(r[5]))
        enc.append((tuple(key), r[6]))
    enc.sort()  # sort by encoded key == sort by (filter columns..., period)

    body = b"".join(struct.pack(fmt, *k, n) for k, n in enc)
    sections[gset] = {
        "offset": offset, "rows": len(enc), "rowSize": row_size,
        "columns": cols, "sortKey": [c for c, _ in cols[:-1]],
        "index": {"every": EVERY,
                  "keys": [list(enc[i][0]) for i in range(0, len(enc), EVERY)]},
    }
    blobs.append(body)
    offset += len(body)

header = json.dumps({"format": "dcb1", "dicts": dicts, "sections": sections},
                    separators=(",", ":")).encode()
with open(OUT, "wb") as f:
    f.write(b"DCB1")
    f.write(struct.pack("<I", len(header)))
    f.write(header)
    for b in blobs:
        f.write(b)

total = 8 + len(header) + offset
print(f"wrote {OUT}: {total:,} bytes   (header {len(header):,} B, data {offset:,} B)")
for g, s in sections.items():
    print(f"  {g:<24} rows={s['rows']:>8,}  rowSize={s['rowSize']:>2}  "
          f"bytes={s['rows']*s['rowSize']:>10,}  indexKeys={len(s['index']['keys'])}")
