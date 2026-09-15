"""Exercise the shipped data loader and package installer without a browser."""
import ast
import asyncio
import json
import sqlite3
import sys
from types import SimpleNamespace
from unittest.mock import AsyncMock

import pytest

from test_datasette_bridge import HERE, extract_python, load


def prepare(options, inputs, tmp_path):
    ns = load()
    fetched = []

    async def fetch(url):
        fetched.append(url)
        return inputs[url]

    files, metadata = asyncio.run(ns["prepare_datasette"](options, fetch, tmp_path))
    return files, metadata, fetched


def test_sql_runs_before_csv_and_json_and_sources_share_data_db(tmp_path):
    options = {"csv": ["https://data/items.csv"], "json": ["https://data/more.json"],
               "sql": ["https://data/one.sql", "https://data/two.sql"]}
    files, _, fetched = prepare(options, {
        "https://data/one.sql": b"create table items (id integer primary key, name text);",
        "https://data/two.sql": b"insert into items values (1, 'seed');",
        "https://data/items.csv": b'id,name\n2,"O\'Brien"\n',
        "https://data/more.json": b'[{"id": 3, "name": "extra"}]',
    }, tmp_path)
    assert files == [str(tmp_path / "data.db")]
    assert fetched == options["sql"] + options["csv"] + options["json"]
    with sqlite3.connect(files[0]) as db:
        assert db.execute("select id, name from items order by id").fetchall() == [
            (1, "seed"), (2, "O'Brien")]
        assert db.execute("select name from more").fetchone() == ("extra",)


def test_repeated_csv_json_names_types_and_delimiters(tmp_path):
    files, _, _ = prepare({"csv": ["https://a/items.csv", "https://b/items.csv"],
                           "json": ["https://c/items.json"]}, {
        "https://a/items.csv": b"name,qty\nWidget,5\n",
        "https://b/items.csv": "name;qty\nCaf\u00e9;12\n".encode("utf-8-sig"),
        "https://c/items.json": b'{"name":"Gadget","qty":7}\n{"name":"Cog","qty":8}\n\n',
    }, tmp_path)
    with sqlite3.connect(files[0]) as db:
        assert db.execute("select name, qty, typeof(qty) from items").fetchone() == (
            "Widget", 5, "integer")
        assert db.execute("select name, qty from items_1").fetchone() == ("Caf\u00e9", 12)
        assert db.execute("select name, qty from items_2").fetchall() == [
            ("Gadget", 7), ("Cog", 8)]


@pytest.mark.parametrize("content,expected,pk", [
    ('[{"a": 1}]', [{"a": 1}], None),
    ('{"a": 1}\n{"a": 2}\n\n', [{"a": 1}, {"a": 2}], None),
    ('{"small": [{"a": 0}], "rows": [{"a": 1}, {"a": 2}]}',
     [{"a": 1}, {"a": 2}], None),
    ('{"first": {"a": 1}, "second": {"a": 2}}',
     [{"a": 1, "_key": "first"}, {"a": 2, "_key": "second"}], "_key"),
])
def test_json_shapes(content, expected, pk):
    assert load()["json_rows"](content) == (expected, pk)


@pytest.mark.parametrize("content", ['{"a": 1}', '[1, 2]', 'null'])
def test_invalid_json_shape_is_rejected(content):
    with pytest.raises(ValueError, match="list of objects"):
        load()["json_rows"](content)


def test_sqlite_names_do_not_overwrite_each_other_or_imports(tmp_path):
    source = tmp_path / "source.db"
    with sqlite3.connect(source) as db:
        db.execute("create table original (value text)")
        db.execute("insert into original values ('preserved')")
    urls = ["https://a/data.db", "https://b/data.db", "https://c/data_2.db"]
    inputs = {url: source.read_bytes() for url in urls}
    inputs["https://a/import.sql"] = b"create table imported (value text);"
    files, _, _ = prepare({"url": urls, "sql": ["https://a/import.sql"]}, inputs, tmp_path)
    assert [file.rsplit("/", 1)[-1] for file in files] == [
        "data_3.db", "data_4.db", "data_2.db", "data.db"]
    for file in files[:-1]:
        with sqlite3.connect(file) as db:
            assert db.execute("select value from original").fetchone() == ("preserved",)


@pytest.mark.parametrize("content", [b'{"title":"Custom title"}', b'title: Custom title\n'])
def test_metadata_and_custom_database_are_used_by_app(tmp_path, content):
    ns = load()
    inputs = {"https://data/metadata": content, "https://data/start.sql":
              b"create table custom (value text); insert into custom values ('hello');"}

    async def fetch(url):
        return inputs[url]

    async def go():
        app = await ns["build_app"]({"metadata": "https://data/metadata",
                                    "sql": ["https://data/start.sql"]}, fetch, tmp_path)
        bridge = ns["ASGIBridge"](app)
        await bridge.startup()
        try:
            home = await bridge.handle("GET", "/app/", "", [])
            rows = await bridge.handle("GET", "/app/data/custom.json", "_shape=array", [])
            assert b"Custom title" in home["body"]
            assert json.loads(rows["body"]) == [{"rowid": 1, "value": "hello"}]
            assert "demo" not in ns["STATE"]["ds"].databases
        finally:
            await bridge.shutdown()

    asyncio.run(go())


@pytest.mark.parametrize("ref", [None, "pre", "0.65.2"])
def test_package_selection_and_plugin_install_order(monkeypatch, ref):
    install = AsyncMock()
    monkeypatch.setitem(sys.modules, "micropip", SimpleNamespace(install=install))
    wheels = ["https://demo/vendor/datasette-1.0a31-py3-none-any.whl",
              "https://demo/vendor/sqlite_utils-4.0a1-py3-none-any.whl"]
    plugins = ["datasette-plugin", "https://demo/plugin.whl"]
    ns = {"_datasette_options_json": json.dumps({"ref": ref, "install": plugins}),
          "_datasette_wheels_json": json.dumps(wheels)}
    code = compile(extract_python("DATASETTE_INSTALL_PY", HERE / "datasette-startup.js"),
                   "datasette-install", "exec", flags=ast.PyCF_ALLOW_TOP_LEVEL_AWAIT)
    asyncio.run(eval(code, ns))
    calls = install.call_args_list
    assert calls[0].args == (wheels[1:] if ref else wheels,)
    if ref:
        assert calls[1].args == ("datasette" if ref == "pre" else "datasette==" + ref,)
        assert calls[1].kwargs == {"pre": ref == "pre"}
    assert [call.args[0] for call in calls[-2:]] == plugins


@pytest.mark.parametrize("as_json", [False, True])
def test_config_settings_plugins_assets_and_metadata(tmp_path, as_json):
    import yaml
    config = {
        "settings": {"default_page_size": 2, "base_url": "/wrong/", "num_sql_threads": 4},
        "plugins": {"example": {"message": "configured"}},
        "extra_css_urls": ["https://assets.example/table.css"],
        "extra_js_urls": ["https://assets.example/table.js"],
    }
    config_bytes = (json.dumps(config) if as_json else yaml.safe_dump(config)).encode()
    ns = load()

    async def fetch(url):
        return config_bytes if url.endswith("config") else b'{"title":"Separate metadata"}'

    async def go():
        app = await ns["build_app"]({"config": "https://data/config",
                                    "metadata": "https://data/metadata"}, fetch, tmp_path)
        ds = ns["STATE"]["ds"]
        assert ds.setting("default_page_size") == 2
        assert ds.setting("base_url") == "/app/"
        assert ds.setting("num_sql_threads") == 0
        assert ds.plugin_config("example") == {"message": "configured"}
        bridge = ns["ASGIBridge"](app)
        await bridge.startup()
        try:
            response = await bridge.handle("GET", "/app/", "", [])
            assert b"Separate metadata" in response["body"]
            assert b"https://assets.example/table.css" in response["body"]
            assert b"https://assets.example/table.js" in response["body"]
        finally:
            await bridge.shutdown()

    asyncio.run(go())


def test_config_rejects_non_object(tmp_path):
    async def fetch(url):
        return b"- not\n- an object\n"

    with pytest.raises(ValueError, match="config must contain a JSON or YAML object"):
        asyncio.run(load()["build_app"]({"config": "https://data/config"}, fetch, tmp_path))


def test_database_named_app_keeps_table_and_export_links(tmp_path):
    source = tmp_path / "source.db"
    with sqlite3.connect(source) as db:
        db.execute("create table app (id integer primary key)")
    ns = load()

    async def fetch(url):
        return source.read_bytes()

    async def go():
        app = await ns["build_app"]({"url": ["https://data/app.db"]}, fetch, tmp_path)
        bridge = ns["ASGIBridge"](app)
        await bridge.startup()
        try:
            db = await bridge.handle("GET", "/app/app", "", [])
            table = await bridge.handle("GET", "/app/app/app", "", [])
            assert b'href="/app/app/app"' in db["body"]
            assert b'href="/app/app/app.json"' in table["body"]
        finally:
            await bridge.shutdown()

    asyncio.run(go())
