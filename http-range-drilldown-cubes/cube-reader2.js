// cube-reader2.js — zero-dependency client for DCB1 (raw) and DCB2
// (compressed) drilldown-cube files. Works in any modern browser and Node 18+:
// fetch, DataView, and the native DecompressionStream API. Nothing else.
//
//   const cube = await CubeReader.open(url);
//   await cube.query("day:agency", { agency: "NYPD" }, { groupBy: "d" });
//
// DCB2 differences from DCB1: the JSON header is deflate-raw compressed, and
// each index-aligned block of rows is stored column-transposed and
// deflate-raw compressed, with per-block byte offsets in the sparse index.
// A query is still one in-memory binary search plus one HTTP range request;
// the fetched bytes are just denser.

const DAY_MS = 86400000;
const SIZES = { u8: 1, u16: 2, u32: 4, "date-u16": 2, "year-u16": 2 };

const isoFromDays = (d) => new Date(d * DAY_MS).toISOString().slice(0, 10);
const daysFromIso = (iso) => Math.floor(Date.parse(iso + "T00:00:00Z") / DAY_MS);

function cmp(a, b) {
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return a[i] < b[i] ? -1 : 1;
  }
  return 0;
}

async function inflateRaw(bytes) {
  const stream = new Blob([bytes]).stream()
    .pipeThrough(new DecompressionStream("deflate-raw"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
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
  static async open(url, { speculative = 131072 } = {}) {
    const stats = { requests: 0, bytesFetched: 0 };
    const first = await rangeFetch(url, 0, speculative - 1, stats);
    const magic = new TextDecoder().decode(first.slice(0, 4));
    if (magic !== "DCB1" && magic !== "DCB2") throw new Error("not a DCB file");
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
    if (magic === "DCB2") headerBytes = await inflateRaw(headerBytes);
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

    // Leading equality filters (a prefix of the sort key) bound the byte
    // range; a {gte,lt} range on the next key column narrows it further.
    // Everything is re-checked after decode, so edges and extras stay exact.
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
      if (id === undefined) return [];
      eqIds.push(id);
    }
    const lo = [...eqIds], hi = [...eqIds];
    if (bound?.lo !== undefined) lo.push(bound.lo);
    if (bound?.hi !== undefined) hi.push(bound.hi); // lt exclusive: pad low
    while (lo.length < key.length) lo.push(-1);
    while (hi.length < key.length) hi.push(Infinity);

    // Binary search the in-memory sparse index for the covering block range.
    const { keys: idx, every } = sec.index;
    let a = 0, b = idx.length;
    while (a < b) { const m = (a + b) >> 1; cmp(idx[m], lo) < 0 ? (a = m + 1) : (b = m); }
    let c = 0, d = idx.length;
    while (c < d) { const m = (c + d) >> 1; cmp(idx[m], hi) <= 0 ? (c = m + 1) : (d = m); }
    const b0 = Math.max(0, a - 1), b1 = Math.min(idx.length, c);
    if (b1 <= b0) return [];

    // One contiguous range request either way; DCB2 fetches denser bytes.
    const base = this.dataStart + sec.offset;
    const blocks = []; // { view, rowsInBlock, layout }
    if (sec.codec === "deflate-raw") {
      const offs = sec.index.offsets;
      const relStart = offs[b0];
      const relEnd = b1 < offs.length ? offs[b1] : sec.bytes;
      const bytes = await rangeFetch(this.url, base + relStart, base + relEnd - 1, this.stats);
      await Promise.all(
        Array.from({ length: b1 - b0 }, async (_, i) => {
          const blk = b0 + i;
          const s = offs[blk] - relStart;
          const e = (blk + 1 < offs.length ? offs[blk + 1] : sec.bytes) - relStart;
          const payload = await inflateRaw(bytes.subarray(s, e));
          blocks[i] = {
            view: new DataView(payload.buffer, payload.byteOffset, payload.byteLength),
            rowsInBlock: Math.min(every, sec.rows - blk * every),
            layout: sec.layout ?? "columnar",
          };
        })
      );
    } else {
      const startRow = b0 * every, endRow = Math.min(sec.rows, b1 * every);
      const bytes = await rangeFetch(
        this.url, base + startRow * sec.rowSize, base + endRow * sec.rowSize - 1, this.stats
      );
      blocks.push({
        view: new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength),
        rowsInBlock: endRow - startRow,
        layout: "row",
      });
    }

    // Field layout within a row (row-major) and per-block column offsets
    // (columnar) both derive from the declared column order.
    let off = 0;
    const fields = sec.columns.map(([name, type]) => {
      const f = { name, type, size: SIZES[type], rowOff: off };
      off += f.size;
      return f;
    });
    const makeGet = ({ view, rowsInBlock, layout }) => {
      const colOff = {};
      if (layout === "columnar") {
        let o = 0;
        for (const f of fields) { colOff[f.name] = o; o += f.size * rowsInBlock; }
      }
      return (i, f) => {
        const p = layout === "columnar" ? colOff[f.name] + i * f.size
                                        : i * sec.rowSize + f.rowOff;
        return f.size === 1 ? view.getUint8(p)
             : f.size === 4 ? view.getUint32(p, true)
             : view.getUint16(p, true);
      };
    };

    // Residual predicates: everything the byte range didn't already guarantee.
    const preds = [];
    key.forEach((col, i) => {
      const f = filters[col];
      if (f === undefined) return;
      const field = fields.find((x) => x.name === col);
      if (typeof f === "object") {
        const gte = f.gte !== undefined ? this._encode(sec, col, f.gte) : -Infinity;
        const lt = f.lt !== undefined ? this._encode(sec, col, f.lt) : Infinity;
        preds.push({ field, test: (v) => v >= gte && v < lt });
      } else {
        const id = i < eqIds.length ? eqIds[i] : this._encode(sec, col, f);
        preds.push({ field, test: (v) => v === id });
      }
    });

    const gField = groupBy && fields.find((x) => x.name === groupBy);
    const sField = fields.find((x) => x.name === sum);
    if (groupBy && !gField) throw new Error(`no column ${groupBy} in this section`);

    const groups = new Map();
    const rows = [];
    for (const blk of blocks) {
      const get = makeGet(blk);
      outer: for (let i = 0; i < blk.rowsInBlock; i++) {
        for (const p of preds) if (!p.test(get(i, p.field))) continue outer;
        if (groupBy) {
          const k = get(i, gField);
          groups.set(k, (groups.get(k) ?? 0) + get(i, sField));
        } else {
          const obj = {};
          for (const f of fields) obj[f.name] = this._decode(f.name, f.type, get(i, f));
          rows.push(obj);
        }
      }
    }
    if (!groupBy) return rows;
    return [...groups.entries()]
      .sort(([x], [y]) => x - y)
      .map(([k, v]) => ({ [groupBy]: this._decode(groupBy, gField.type, k), [sum]: v }));
  }
}
