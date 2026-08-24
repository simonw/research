// validate_nyc.js — query the converted NYC 311 cube with the unchanged
// cube-reader.js over real HTTP range requests; log the cost of each
// dashboard interaction from the article.
import { createServer } from "node:http";
import { statSync, createReadStream, writeFileSync } from "node:fs";
import { CubeReader } from "./cube-reader.js";

const FILE = "nyc311-cube-v15.dcb1";
const size = statSync(FILE).size;
const log = [];
const server = createServer((req, res) => {
  const m = /bytes=(\d+)-(\d*)/.exec(req.headers.range || "");
  const start = m ? +m[1] : 0;
  const end = m ? Math.min(m[2] ? +m[2] : size - 1, size - 1) : size - 1;
  log.push(end - start + 1);
  res.writeHead(m ? 206 : 200, {
    "content-range": `bytes ${start}-${end}/${size}`,
    "content-length": end - start + 1,
  });
  createReadStream(FILE, { start, end }).pipe(res);
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const url = `http://127.0.0.1:${server.address().port}/${FILE}`;

let mark = 0;
const report = (label) => {
  const reqs = log.slice(mark);
  mark = log.length;
  const bytes = reqs.reduce((a, b) => a + b, 0);
  console.log(`\n# ${label}`);
  console.log(`  ${reqs.length} request(s), ${(bytes / 1024).toFixed(1)} KB  (${(100 * bytes / size).toFixed(3)}% of the 130MB file)`);
  return bytes;
};

const FULL = "agency+complaint+borough+channel";
const cube = await CubeReader.open(url);
report("open(): header (149KB JSON: dicts + section table + sparse indexes)");

let r = await cube.query(`all:${FULL}`, {}, { groupBy: "agency" });
report("all-time agency leaderboard (whole `all` section, grouped client-side)");
console.log("   top:", JSON.stringify(r.sort((a, b) => b.n - a.n).slice(0, 3)));

r = await cube.query("day:agency", { agency: "NYPD" }, { groupBy: "d" });
report('CLICK "NYPD": full 13.7-year daily line  [the article\'s ~260KB interaction]');
console.log(`   ${r.length} points  ${JSON.stringify(r.slice(0, 2))}`);
writeFileSync("js_nypd_daily.json", JSON.stringify(r));

r = await cube.query(`year:${FULL}`, { agency: "NYPD" }, { groupBy: "complaint" });
report("complaint leaderboard while NYPD selected (yearly section slice)");
console.log("   top:", JSON.stringify(r.sort((a, b) => b.n - a.n).slice(0, 3)));
writeFileSync("js_nypd_complaints.json",
  JSON.stringify(r.sort((a, b) => a.complaint < b.complaint ? -1 : 1)));

r = await cube.query("day:agency",
  { agency: "NYPD", d: { gte: "2013-01-01", lt: "2014-01-01" } }, { groupBy: "d" });
report("brush 2013 on the NYPD line (sortKey [agency,d] => range-bounded fetch)");
console.log(`   ${r.length} points  ${JSON.stringify(r.slice(0, 2))}`);

r = await cube.query("day:agency+complaint",
  { agency: "NYPD", complaint: "Illegal Parking" }, { groupBy: "d" });
report("NYPD + Illegal Parking daily line (2-dim section slice)");
console.log(`   ${r.length} points`);

r = await cube.query(`day:${FULL}`,
  { agency: "NYPD", complaint: "Illegal Parking", borough: "BROOKLYN", channel: "PHONE" });
report("fully drilled down: 4 filters, raw daily rows from the 29.5MB section");
console.log(`   ${r.length} rows  ${JSON.stringify(r.slice(0, 1))}`);

const total = log.reduce((a, b) => a + b, 0);
console.log(`\nTOTAL cold session, 7 interactions: ${log.length} requests, ${(total / 1024).toFixed(0)} KB = ${(100 * total / size).toFixed(2)}% of the file`);
server.close();
