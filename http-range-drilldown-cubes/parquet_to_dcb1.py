#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["pyarrow", "numpy"]
# ///
"""Convert a Parquet drilldown cube (with an explicit grouping_set id column)
into DCB1. Discovers each set's active dimensions and time grain from the data,
then dictionary-encodes, sorts, and packs fixed-width sections with a sparse
key index in the JSON header. Output is readable by the unchanged
cube-reader.js.
"""
import json, struct, sys, time
import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

SRC = sys.argv[1] if len(sys.argv) > 1 else "nyc311-cube-v15.parquet"
OUT = sys.argv[2] if len(sys.argv) > 2 else "nyc311-cube-v15.dcb1"

DIMS = ["agency", "complaint", "borough", "channel"]  # canonical sort order
TIME, MEASURE, SETCOL = "d", "n", "grouping_set"
MAX_INDEX_ENTRIES = 1024  # per section; `every` doubles from 256 until it fits

t0 = time.time()
t = pq.read_table(SRC)
N = t.num_rows

# --- global dictionaries (sorted: id order == value order) and encoded columns
dicts, codes = {}, {}
for col in DIMS:
    arr = t[col].combine_chunks()
    values = sorted(v for v in pc.unique(arr).to_pylist() if v is not None)
    dicts[col] = values
    gid = {v: i for i, v in enumerate(values)}
    enc = pc.dictionary_encode(arr)
    remap = np.array([gid[v] for v in enc.dictionary.to_pylist()], np.int32)
    idx = enc.indices.fill_null(-1).to_numpy(zero_copy_only=False).astype(np.int32)
    codes[col] = np.where(idx >= 0, remap[np.clip(idx, 0, None)], -1)

days = pc.cast(t[TIME].combine_chunks(), pa.int32()).fill_null(-1) \
         .to_numpy(zero_copy_only=False).astype(np.int32)  # date32 == days since epoch
n = t[MEASURE].combine_chunks().to_numpy(zero_copy_only=False).astype(np.int64)
gs = t[SETCOL].combine_chunks().to_numpy(zero_copy_only=False).astype(np.int16)
assert n.min() >= 0

def dim_dtype(card):
    return ("u8", "u1") if card <= 256 else ("u16", "u2") if card <= 65536 else ("u32", "u4")

def grain(dvals):
    if len(dvals) == 0:
        return "all"
    distinct = np.unique(dvals)
    if len(np.unique(distinct % 7)) > 1:      # 1970-01-01 was a Thursday
        return "day"                          # multiple weekdays => daily grain
    return "year" if len(distinct) <= 40 else "week"

sections, blobs, offset = {}, [], 0
for s in sorted(np.unique(gs)):
    rows = np.flatnonzero(gs == s)
    active = []
    for col in DIMS:
        c = codes[col][rows]
        if (c >= 0).all():
            active.append(col)
        else:
            assert (c < 0).all(), f"set {s}: {col} has mixed NULLs (real NULLs in data?)"
    d = days[rows]
    has_time = (d >= 0).all() and len(d) > 0
    if not has_time:
        assert (d < 0).all(), f"set {s}: {TIME} has mixed NULLs"
    g = grain(d if has_time else [])
    name = f"{g}:{'+'.join(active) or 'overall'}"

    key_arrays = [codes[c][rows] for c in active] + ([d] if has_time else [])
    order = np.lexsort(tuple(reversed(key_arrays))) if key_arrays else np.arange(len(rows))
    key_arrays = [k[order] for k in key_arrays]
    nn = n[rows][order]

    cols, np_fields = [], []
    for c in active:
        tname, npt = dim_dtype(len(dicts[c]))
        cols.append([c, tname]); np_fields.append((c, "<" + npt))
    if has_time:
        cols.append([TIME, "date-u16"]); np_fields.append((TIME, "<u2"))
    ntype = "u16" if nn.max() < 65536 else "u32"
    cols.append([MEASURE, ntype]); np_fields.append((MEASURE, "<u2" if ntype == "u16" else "<u4"))

    rec = np.zeros(len(rows), dtype=np.dtype(np_fields))  # packed, no padding
    for (cname, _), arr in zip(cols[:-1], key_arrays):
        rec[cname] = arr
    rec[MEASURE] = nn
    body = rec.tobytes()

    every = 256
    while -(-len(rows) // every) > MAX_INDEX_ENTRIES:
        every *= 2
    keys = [[int(k[i]) for k in key_arrays] for i in range(0, len(rows), every)]

    sections[name] = {
        "offset": offset, "rows": len(rows), "rowSize": rec.itemsize,
        "columns": cols, "sortKey": [c for c, _ in cols[:-1]],
        "index": {"every": every, "keys": keys},
        "meta": {"grouping_set": int(s), "sumN": int(nn.sum())},
    }
    blobs.append(body)
    offset += len(body)

header = json.dumps({"format": "dcb1", "dicts": dicts, "sections": sections},
                    separators=(",", ":")).encode()
with open(OUT, "wb") as f:
    f.write(b"DCB1"); f.write(struct.pack("<I", len(header))); f.write(header)
    for b in blobs:
        f.write(b)

total = 8 + len(header) + offset
print(f"{SRC.split('/')[-1]}: {N:,} rows -> {OUT}: {total:,} bytes "
      f"(header {len(header):,} B) in {time.time()-t0:.1f}s\n")
print(f"{'section':<42}{'rows':>10}  {'rowSize':>7}  {'MB':>7}  {'every':>5}  {'sum(n)':>12}")
for name, s in sections.items():
    print(f"{name:<42}{s['rows']:>10,}  {s['rowSize']:>7}  "
          f"{s['rows']*s['rowSize']/1e6:>7.1f}  {s['index']['every']:>5}  {s['meta']['sumN']:>12,}")
