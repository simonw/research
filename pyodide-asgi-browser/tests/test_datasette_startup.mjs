import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const context = vm.createContext({ URL });
vm.runInContext(readFileSync(new URL("../datasette-startup.js", import.meta.url), "utf8"), context);
const parse = href => JSON.parse(JSON.stringify(context.parseDatasetteOptions(href)));

test("startup parameters are distinct from the fragment query, preserve repeats and encoding", () => {
  const url = new URL("https://demo.test/sub/datasette.html");
  url.searchParams.append("csv", "items.csv?key=a&b=c");
  url.searchParams.append("csv", "more.csv");
  url.searchParams.append("sql", "setup.sql");
  url.searchParams.append("install", "datasette-example>=1");
  url.searchParams.append("install", "https://plugins.test/example.whl");
  url.searchParams.append("ref", "pre");
  url.searchParams.append("url", "");
  url.hash = "/data?sql=select+1&csv=ignored.csv";
  assert.deepEqual(parse(url.href), {
    csv: ["https://demo.test/sub/items.csv?key=a&b=c", "https://demo.test/sub/more.csv"],
    url: [], json: [], sql: ["https://demo.test/sub/setup.sql"], metadata: null, config: null,
    install: ["datasette-example>=1", "https://plugins.test/example.whl"], ref: "pre",
  });
});

test("GitHub and Gist data/metadata links become raw URLs; install stays a requirement", () => {
  const url = new URL("https://demo.test/datasette.html");
  url.searchParams.set("url", "https://github.com/org/repo/blob/main/data.db?raw=true");
  url.searchParams.set("json", "https://gist.github.com/user/abc123/");
  url.searchParams.set("metadata", "https://github.com/org/repo/blob/main/metadata.yml");
  url.searchParams.set("config", "https://gist.github.com/user/config123");
  url.searchParams.set("install", "datasette-example==1.0");
  const options = parse(url.href);
  assert.deepEqual(options.url, ["https://raw.githubusercontent.com/org/repo/main/data.db"]);
  assert.deepEqual(options.json, ["https://gist.githubusercontent.com/user/abc123/raw"]);
  assert.equal(options.metadata, "https://raw.githubusercontent.com/org/repo/main/metadata.yml");
  assert.equal(options.config, "https://gist.githubusercontent.com/user/config123/raw");
  assert.deepEqual(options.install, ["datasette-example==1.0"]);
});
