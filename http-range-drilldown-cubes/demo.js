// demo.js — serve cube.bin over real HTTP with Range support, query it with
// CubeReader, and report exactly which bytes each dashboard interaction cost.
import { createServer } from "node:http";
import { statSync, createReadStream, writeFileSync } from "node:fs";
import { CubeReader } from "./cube-reader.js";

const FILE = "cube.bin";
const size = statSync(FILE).size;
const log = [];

const server = createServer((req, res) => {
  const m = /bytes=(\d+)-(\d*)/.exec(req.headers.range || "");
  if (!m) {
    log.push({ range: "FULL", bytes: size });
    res.writeHead(200, { "content-length": size });
    createReadStream(FILE).pipe(res);
    return;
  }
  const start = +m[1];
  const end = Math.min(m[2] ? +m[2] : size - 1, size - 1);
  log.push({ range: `${start}-${end}`, bytes: end - start + 1 });
  res.writeHead(206, {
    "content-range": `bytes ${start}-${end}/${size}`,
    "content-length": end - start + 1,
  });
  createReadStream(FILE, { start, end }).pipe(res);
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const url = `http://127.0.0.1:${server.address().port}/cube.bin`;

let mark = 0;
function report(label) {
  const reqs = log.slice(mark);
  mark = log.length;
  const bytes = reqs.reduce((s, r) => s + r.bytes, 0);
  console.log(`\n# ${label}`);
  console.log(
    `  ${reqs.length} request(s), ${bytes.toLocaleString()} bytes ` +
    `(${((100 * bytes) / size).toFixed(2)}% of the ${(size / 1e6).toFixed(1)}MB file)` +
    `   ranges: ${reqs.map((r) => r.range).join("  ")}`
  );
}

const cube = await CubeReader.open(url);
report("open() — magic + length + entire JSON header");

let rows = await cube.query("total:agency");
report("agency leaderboard: whole `total:agency` section");
console.log("  ", JSON.stringify(rows));

const yearly = await cube.query("year:full", { agency: "NYPD" }, { groupBy: "period" });
report("yearly line, agency=NYPD  (coarse grain)");
console.log("  ", JSON.stringify(yearly.slice(0, 4)), `… ${yearly.length} points`);
writeFileSync("cube_yearly_nypd.json", JSON.stringify(yearly));

const daily = await cube.query("day:full", { agency: "NYPD" }, { groupBy: "period" });
report("daily line, agency=NYPD  (fine grain, 192,633-row section)");
console.log("  ", JSON.stringify(daily.slice(0, 2)), `… ${daily.length} points`);

rows = await cube.query("day:full", {
  agency: "NYPD", complaint_type: "Blocked Driveway",
  submission_type: "MOBILE", borough: "BRONX",
});
report("fully drilled down: NYPD + Blocked Driveway + MOBILE + BRONX, daily rows");
console.log("  ", JSON.stringify(rows.slice(0, 2)), `… ${rows.length} rows`);

const brush = await cube.query(
  "day:full",
  { agency: "NYPD", period: { gte: "2013-01-01", lt: "2014-01-01" } },
  { groupBy: "period" }
);
report("brushed: agency=NYPD, day in 2013 (period is last in sort key, so this fetches the NYPD block and filters client-side)");
console.log("  ", JSON.stringify(brush.slice(0, 2)), `… ${brush.length} points`);

const total = log.reduce((s, r) => s + r.bytes, 0);
console.log(
  `\nTOTAL for a cold session of 6 interactions: ${log.length} requests, ` +
  `${total.toLocaleString()} bytes (${((100 * total) / size).toFixed(2)}% of file)`
);
server.close();
