// compare_v1_v2.js — run the same dashboard interactions against the raw DCB1
// file (v1 reader) and the compressed DCB2 file (v2 reader), assert the
// results are byte-identical, and report the fetched bytes side by side.
import { createServer } from "node:http";
import { statSync, createReadStream } from "node:fs";
import { CubeReader as V1 } from "./cube-reader.js";
import { CubeReader as V2 } from "./cube-reader2.js";

const files = { "/v1": "nyc311-cube-v15.dcb1", "/v2": "nyc311-cube-v15.dcb2" };
const server = createServer((req, res) => {
  const path = files[req.url];
  const size = statSync(path).size;
  const m = /bytes=(\d+)-(\d*)/.exec(req.headers.range || "");
  const start = m ? +m[1] : 0;
  const end = m ? Math.min(m[2] ? +m[2] : size - 1, size - 1) : size - 1;
  res.writeHead(m ? 206 : 200, {
    "content-range": `bytes ${start}-${end}/${size}`,
    "content-length": end - start + 1,
  });
  createReadStream(path, { start, end }).pipe(res);
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}`;

const FULL = "agency+complaint+borough+channel";
const Q = [
  ["all-time agency leaderboard", `all:${FULL}`, {}, { groupBy: "agency" }],
  ['click "NYPD": 13.7y daily line', "day:agency", { agency: "NYPD" }, { groupBy: "d" }],
  ["complaint leaderboard @ NYPD", `year:${FULL}`, { agency: "NYPD" }, { groupBy: "complaint" }],
  ["brush 2013 on the NYPD line", "day:agency",
    { agency: "NYPD", d: { gte: "2013-01-01", lt: "2014-01-01" } }, { groupBy: "d" }],
  ["NYPD + Illegal Parking daily", "day:agency+complaint",
    { agency: "NYPD", complaint: "Illegal Parking" }, { groupBy: "d" }],
  ["4-filter drilldown, raw rows", `day:${FULL}`,
    { agency: "NYPD", complaint: "Illegal Parking", borough: "BROOKLYN", channel: "PHONE" }, {}],
];

const c1 = await V1.open(`${base}/v1`);
const c2 = await V2.open(`${base}/v2`);
const kb = (b) => (b / 1024).toFixed(1).padStart(9);
const row = (label, b1, b2, note = "") =>
  console.log(`${label.padEnd(32)}${kb(b1)} KB${kb(b2)} KB   ${note}`);

console.log(`${"interaction".padEnd(32)}${"v1 raw".padStart(12)}${"v2 deflate".padStart(12)}   results`);
row("open() [header]", c1.stats.bytesFetched, c2.stats.bytesFetched);
let m1 = c1.stats.bytesFetched, m2 = c2.stats.bytesFetched, allOk = true;
for (const [label, sec, f, o] of Q) {
  const r1 = await c1.query(sec, f, o);
  const r2 = await c2.query(sec, f, o);
  const ok = JSON.stringify(r1) === JSON.stringify(r2);
  allOk &&= ok;
  row(label, c1.stats.bytesFetched - m1, c2.stats.bytesFetched - m2,
      ok ? "identical" : "MISMATCH");
  m1 = c1.stats.bytesFetched;
  m2 = c2.stats.bytesFetched;
}
row("TOTAL cold session", m1, m2,
    `(files: ${(statSync(files["/v1"]).size / 1e6).toFixed(0)}MB vs ${(statSync(files["/v2"]).size / 1e6).toFixed(0)}MB)`);

// Back-compat: the v2 reader must read v1 raw files unchanged.
const c12 = await V2.open(`${base}/v1`);
const a = await c12.query("day:agency", { agency: "NYPD" }, { groupBy: "d" });
const b = await c1.query("day:agency", { agency: "NYPD" }, { groupBy: "d" });
console.log(`\nv2 reader on the v1 file: ${JSON.stringify(a) === JSON.stringify(b) ? "identical (back-compat OK)" : "MISMATCH"}`);
console.log(allOk ? "All interactions returned byte-identical results across formats."
                  : "MISMATCHES FOUND");
process.exitCode = allOk ? 0 : 1;
server.close();
