// cube-reader.js — zero-dependency client for DCB1 drilldown-cube files.
// Works in any browser and in Node 18+ (native fetch, DataView, nothing else).
//
//   const cube = await CubeReader.open(url);              // 1 range request
//   await cube.query("total:agency");                     // whole tiny section
//   await cube.query("day:full", { agency: "NYPD" },      // 1 range request
//                    { groupBy: "period" });
//
// Filters that form a prefix of the section's sort key bound the byte range
// (that's the contiguity guarantee from the file layout). Any other filters,
// including { gte, lt } ranges, are applied after decoding the fetched slice.

const DAY_MS = 86400000;

const isoFromDays = (d) => new Date(d * DAY_MS).toISOString().slice(0, 10);
const daysFromIso = (iso) => Math.floor(Date.parse(iso + "T00:00:00Z") / DAY_MS);

function cmp(a, b) {
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return a[i] < b[i] ? -1 : 1;
  }
  return 0;
}

async function rangeFetch(url, start, end, stats) {
  const res = await fetch(url, { headers: { Range: `bytes=${start}-${end}` } });
  if (res.status !== 206 && res.status !== 200) {
    throw new Error(`range fetch failed: HTTP ${res.status}`);
  }
  const buf = new Uint8Array(await res.arrayBuffer());
  stats.requests++;
  stats.bytesFetched += buf.byteLength;
  return buf;
}

export class CubeReader {
  static async open(url, { speculative = 65536 } = {}) {
    const stats = { requests: 0, bytesFetched: 0 };
    // One speculative read usually covers magic + length + entire header.
    const first = await rangeFetch(url, 0, speculative - 1, stats);
    if (new TextDecoder().decode(first.slice(0, 4)) !== "DCB1") {
      throw new Error("not a DCB1 file");
    }
    const headerLen = new DataView(first.buffer, first.byteOffset + 4, 4).getUint32(0, true);
    let headerBytes;
    if (8 + headerLen <= first.byteLength) {
      headerBytes = first.slice(8, 8 + headerLen);
    } else {
      const rest = await rangeFetch(url, first.byteLength, 8 + headerLen - 1, stats);
      headerBytes = new Uint8Array(headerLen);
      headerBytes.set(first.slice(8));
      headerBytes.set(rest, first.byteLength - 8);
    }
    const reader = new CubeReader();
    reader.url = url;
    reader.header = JSON.parse(new TextDecoder().decode(headerBytes));
    reader.dataStart = 8 + headerLen;
    reader.stats = stats;
    reader._toId = {};
    for (const [col, values] of Object.entries(reader.header.dicts)) {
      reader._toId[col] = new Map(values.map((v, i) => [v, i]));
    }
    return reader;
  }

  sections() {
    return Object.keys(this.header.sections);
  }

  _encode(sec, col, value) {
    const type = sec.columns.find(([name]) => name === col)?.[1];
    if (!type) throw new Error(`no column ${col} in this section`);
    if (type === "date-u16") return daysFromIso(value);
    if (type === "year-u16") return +value;
    return this._toId[col].get(value); // undefined => value not in dictionary
  }

  _decode(col, type, v) {
    if (type === "date-u16") return isoFromDays(v);
    if (type === "year-u16") return v;
    return this.header.dicts[col] ? this.header.dicts[col][v] : v;
  }

  async query(sectionName, filters = {}, { groupBy, sum = "n" } = {}) {
    const sec = this.header.sections[sectionName];
    if (!sec) throw new Error(`no such section: ${sectionName}`);
    const key = sec.sortKey;

    // Split filters: leading equality filters (the sort-key prefix) narrow the
    // byte range; a {gte,lt} range on the *next* key column narrows it further;
    // everything is re-checked after decode, so edges and extras stay correct.
    const eqIds = [];
    let bound = null;
    for (const col of key) {
      const f = filters[col];
      if (f === undefined) break;
      if (typeof f === "object") {
        bound = {
          lo: f.gte !== undefined ? this._encode(sec, col, f.gte) : undefined,
          hi: f.lt !== undefined ? this._encode(sec, col, f.lt) : undefined,
        };
        break;
      }
      const id = this._encode(sec, col, f);
      if (id === undefined) return []; // filter value doesn't exist anywhere
      eqIds.push(id);
    }

    const lo = [...eqIds], hi = [...eqIds];
    if (bound?.lo !== undefined) lo.push(bound.lo);
    if (bound?.hi !== undefined) hi.push(bound.hi); // lt is exclusive: pad low
    while (lo.length < key.length) lo.push(-1);
    while (hi.length < key.length) hi.push(Infinity);

    // Binary search the in-memory sparse index for the covering row range.
    const { keys: idx, every } = sec.index;
    let a = 0, b = idx.length;
    while (a < b) { const m = (a + b) >> 1; cmp(idx[m], lo) < 0 ? (a = m + 1) : (b = m); }
    const startRow = Math.max(0, a - 1) * every;
    let c = 0, d = idx.length;
    while (c < d) { const m = (c + d) >> 1; cmp(idx[m], hi) <= 0 ? (c = m + 1) : (d = m); }
    const endRow = Math.min(sec.rows, c * every);
    if (endRow <= startRow) return [];

    // The one data fetch: a single contiguous byte range.
    const base = this.dataStart + sec.offset;
    const bytes = await rangeFetch(
      this.url, base + startRow * sec.rowSize, base + endRow * sec.rowSize - 1, this.stats
    );
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);

    // Precompute field offsets within a row.
    const size = { u8: 1, u16: 2, u32: 4, "date-u16": 2, "year-u16": 2 };
    let off = 0;
    const fields = sec.columns.map(([name, type]) => {
      const f = { name, type, off };
      off += size[type];
      return f;
    });
    const get = (rowOff, f) =>
      f.type === "u8" ? view.getUint8(rowOff + f.off)
      : f.type === "u32" ? view.getUint32(rowOff + f.off, true)
      : view.getUint16(rowOff + f.off, true);

    // Residual predicates: everything the byte range didn't already guarantee.
    const preds = [];
    key.forEach((col, i) => {
      const f = filters[col];
      if (f === undefined) return;
      const field = fields.find((x) => x.name === col);
      if (typeof f === "object") {
        const gte = f.gte !== undefined ? this._encode(sec, col, f.gte) : -Infinity;
        const lt = f.lt !== undefined ? this._encode(sec, col, f.lt) : Infinity;
        preds.push((r) => { const v = get(r, field); return v >= gte && v < lt; });
      } else if (i < eqIds.length) {
        const id = eqIds[i];
        preds.push((r) => get(r, field) === id); // cheap edge-trimming re-check
      } else {
        const id = this._encode(sec, col, f);
        preds.push((r) => get(r, field) === id); // non-prefix filter: post-scan
      }
    });

    const gField = groupBy && fields.find((x) => x.name === groupBy);
    const sField = fields.find((x) => x.name === sum);
    if (groupBy && !gField) throw new Error(`no column ${groupBy} in this section`);

    const groups = new Map();
    const rows = [];
    const n = endRow - startRow;
    outer: for (let i = 0; i < n; i++) {
      const r = i * sec.rowSize;
      for (const p of preds) if (!p(r)) continue outer;
      if (groupBy) {
        const k = get(r, gField);
        groups.set(k, (groups.get(k) ?? 0) + get(r, sField));
      } else {
        const obj = {};
        for (const f of fields) obj[f.name] = this._decode(f.name, f.type, get(r, f));
        rows.push(obj);
      }
    }
    if (!groupBy) return rows;
    return [...groups.entries()]
      .sort(([x], [y]) => x - y)
      .map(([k, v]) => ({ [groupBy]: this._decode(groupBy, gField.type, k), [sum]: v }));
  }
}
