// URL configuration and data loading for the Datasette demo. Python blocks are
// extracted by the unit tests, just like the ASGI bridge and application blocks.
function datasetteSourceUrl(value, baseUrl) {
  const url = new URL(value, baseUrl);
  const github = url.pathname.match(/^\/([^/]+)\/([^/]+)\/blob\/(.+)$/);
  if (url.hostname === "github.com" && github) {
    return `https://raw.githubusercontent.com/${github[1]}/${github[2]}/${github[3]}`;
  }
  const gist = url.pathname.match(/^\/([^/]+)\/([^/]+)\/?$/);
  if (url.hostname === "gist.github.com" && gist) {
    return `https://gist.githubusercontent.com/${gist[1]}/${gist[2]}/raw`;
  }
  return url.href;
}

function parseDatasetteOptions(href) {
  const params = new URL(href).searchParams;
  const options = {};
  for (const name of ["url", "csv", "json", "sql"]) {
    options[name] = params.getAll(name).filter(Boolean).map(value => datasetteSourceUrl(value, href));
  }
  for (const name of ["metadata", "config"]) {
    const value = params.get(name);
    options[name] = value ? datasetteSourceUrl(value, href) : null;
  }
  options.install = params.getAll("install").filter(Boolean);
  options.ref = params.get("ref") || null;
  return options;
}

const DATASETTE_INSTALL_PY = String.raw`
import json
from urllib.parse import urlsplit
import micropip

DATASETTE_OPTIONS = json.loads(_datasette_options_json)
wheels = json.loads(_datasette_wheels_json)
ref = DATASETTE_OPTIONS.get("ref")
if ref:
    # Select the requested version before importing Datasette. Installing the
    # vendored Datasette first would make micropip reject a different version.
    wheels = [w for w in wheels if not urlsplit(w).path.rsplit("/", 1)[-1].startswith("datasette-")]
await micropip.install(wheels)
if ref:
    requirement = "datasette" if ref == "pre" else "datasette==" + ref
    await micropip.install(requirement, pre=(ref == "pre"))
for requirement in DATASETTE_OPTIONS.get("install", []):
    await micropip.install(requirement)
`;

const DATASETTE_STARTUP_PY = String.raw`
import csv
import io
import json
import sqlite3
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit


def source_name(url, database=False):
    filename = unquote(urlsplit(url).path.rsplit("/", 1)[-1])
    name = Path(filename).stem if database else filename.split(".")[0]
    name = name.strip().replace("/", "_").replace("\\", "_")
    return name if name not in ("", ".", "..") else ("database" if database else "table")


def database_names(urls, has_imports):
    names = [source_name(url, database=True) for url in urls]
    reserved = set(names)
    used = {"_memory", "_internal"}
    if has_imports:
        used.add("data")
    result = []
    for base in names:
        name, suffix = base, 2
        if name in used:
            while True:
                name = "{}_{}".format(base, suffix)
                suffix += 1
                if name not in used and name not in reserved:
                    break
        used.add(name)
        result.append(name)
    return result


def json_rows(content):
    try:
        rows = json.loads(content)
    except json.JSONDecodeError:
        rows = [json.loads(line) for line in content.splitlines() if line.strip()]
    pk = None
    if isinstance(rows, dict) and all(isinstance(value, dict) for value in rows.values()):
        rows = [dict(value, _key=key) for key, value in rows.items()]
        pk = "_key"
    elif isinstance(rows, dict):
        lists = [value for value in rows.values() if isinstance(value, list)
                 and value and all(isinstance(row, dict) for row in value)]
        if lists:
            rows = max(lists, key=len)
    if not isinstance(rows, list) or not all(isinstance(row, dict) for row in rows):
        raise ValueError("JSON data must contain a list of objects")
    return rows, pk


async def prepare_datasette(options, fetch_bytes, directory="."):
    """Download inputs into Pyodide's volatile filesystem before app startup."""
    directory = Path(directory)
    files = []
    urls = options.get("url", [])
    has_imports = any(options.get(kind) for kind in ("csv", "json", "sql"))
    for name, url in zip(database_names(urls, has_imports), urls):
        path = directory / (name + ".db")
        path.write_bytes(await fetch_bytes(url))
        files.append(str(path))

    if has_imports:
        import sqlite_utils
        from sqlite_utils.utils import TypeTracker

        path = directory / "data.db"
        connection = sqlite3.connect(str(path))
        try:
            # SQL always runs first, even when csv/json precede it in the URL.
            for url in options.get("sql", []):
                connection.executescript((await fetch_bytes(url)).decode("utf-8-sig"))
            db = sqlite_utils.Database(connection)
            table_names = set()
            for kind in ("csv", "json"):
                for url in options.get(kind, []):
                    base = source_name(url)
                    table, suffix = base, 1
                    while table in table_names:
                        table = "{}_{}".format(base, suffix)
                        suffix += 1
                    table_names.add(table)
                    content = (await fetch_bytes(url)).decode("utf-8-sig")
                    if kind == "csv":
                        limit = sys.maxsize
                        while True:
                            try:
                                csv.field_size_limit(limit)
                                break
                            except OverflowError:
                                limit //= 10
                        try:
                            delimiter = csv.Sniffer().sniff(content[:8192], delimiters=",;").delimiter
                        except csv.Error:
                            delimiter = ","
                        reader = csv.DictReader(io.StringIO(content), delimiter=delimiter)
                        tracker = TypeTracker()
                        db[table].insert_all(tracker.wrap(reader), alter=True)
                        if db[table].exists():
                            db[table].transform(types=tracker.types)
                        elif reader.fieldnames:
                            db[table].create({name: str for name in reader.fieldnames})
                    else:
                        rows, pk = json_rows(content)
                        db[table].insert_all(rows, pk=pk, alter=True)
            connection.commit()
        finally:
            connection.close()
        files.append(str(path))

    metadata = await load_datasette_document(options.get("metadata"), fetch_bytes, "metadata")
    return files, metadata


async def load_datasette_document(url, fetch_bytes, name):
    if not url:
        return None
    from datasette.utils import parse_metadata
    document = parse_metadata((await fetch_bytes(url)).decode("utf-8-sig"))
    if not isinstance(document, dict):
        raise ValueError("{} must contain a JSON or YAML object".format(name))
    return document
`;
