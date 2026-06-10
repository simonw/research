"""
End-to-end demo: Datasette serving a DuckDB file through its real ASGI app.

Proves the adapter works (queries return correct JSON) AND that the two
safety properties survive: read-only/lockdown and the query time limit.
"""

import asyncio
import os
import tempfile

import duckdb
import httpx

import datasette_duckdb
from datasette.app import Datasette


def build_db():
    workdir = tempfile.mkdtemp(prefix="datasette_duck_")
    path = os.path.join(workdir, "demo.duckdb")
    con = duckdb.connect(path)
    con.execute("CREATE TABLE planets (id INTEGER, name TEXT, moons INTEGER)")
    con.execute(
        "INSERT INTO planets VALUES "
        "(1,'Mercury',0),(2,'Venus',0),(3,'Earth',1),(4,'Mars',2)"
    )
    con.execute("CREATE TABLE moons (id INTEGER, planet TEXT, name TEXT)")
    con.execute("INSERT INTO moons VALUES (1,'Earth','Luna'),(2,'Mars','Phobos')")
    con.close()
    return path


async def main():
    datasette_duckdb.install()
    path = build_db()

    ds = Datasette(memory=True)
    db = datasette_duckdb.DuckDBDatabase(ds, path)
    ds.add_database(db, name="demo")
    # Tight time limit so we can demonstrate interruption quickly.
    ds.sql_time_limit_ms = 500

    await ds.invoke_startup()
    transport = httpx.ASGITransport(app=ds.app())
    async with httpx.AsyncClient(
        transport=transport, base_url="http://t", follow_redirects=True
    ) as client:

        print("=== 1. Table introspection (homepage JSON) ===")
        r = await client.get("/demo.json")
        tables = sorted(t["name"] for t in r.json()["tables"])
        print("   status", r.status_code, "tables:", tables)

        print("\n=== 2. Browse a table: /demo/planets.json ===")
        r = await client.get("/demo/planets.json?_shape=array")
        print("   status", r.status_code)
        for row in r.json():
            print("   ", row)

        print("\n=== 3. Ad-hoc SQL query with a JOIN ===")
        sql = "select p.name as planet, m.name as moon from planets p join moons m on m.planet = p.name order by 1"
        r = await client.get("/demo.json", params={"sql": sql, "_shape": "array"})
        print("   status", r.status_code)
        for row in r.json():
            print("   ", row)

        print("\n=== 4. Aggregate query ===")
        r = await client.get(
            "/demo.json",
            params={"sql": "select sum(moons) as total_moons from planets", "_shape": "array"},
        )
        print("   status", r.status_code, "->", r.json())

        print("\n=== 5. SAFETY: write attempt is blocked (read-only) ===")
        r = await client.get(
            "/demo.json", params={"sql": "insert into planets values (9,'X',0)"}
        )
        body = r.json()
        print("   status", r.status_code, "ok:", body.get("ok"), "error:", body.get("error"))

        print("\n=== 6. SAFETY: filesystem escape blocked (COPY TO) ===")
        r = await client.get(
            "/demo.json",
            params={"sql": "copy planets to '/tmp/exfil_demo.csv' (header, format csv)"},
        )
        body = r.json()
        print("   status", r.status_code, "ok:", body.get("ok"), "error:", (body.get("error") or "")[:70])
        print("   /tmp/exfil_demo.csv exists:", os.path.exists("/tmp/exfil_demo.csv"))

        print("\n=== 7. SAFETY: arbitrary file read blocked (read_csv) ===")
        r = await client.get(
            "/demo.json",
            params={"sql": "select * from read_csv('/etc/hostname', columns={'l':'VARCHAR'}, header=false)"},
        )
        body = r.json()
        print("   status", r.status_code, "ok:", body.get("ok"), "error:", (body.get("error") or "")[:70])

        print("\n=== 8. SAFETY: time limit interrupts a heavy query ===")
        heavy = "select count(*) from range(1000000) a, range(1000000) b where (a.range*b.range)%7=0"
        import time as _t
        start = _t.perf_counter()
        r = await client.get("/demo.json", params={"sql": heavy})
        body = r.json()
        print(
            f"   status {r.status_code} ok: {body.get('ok')} after {_t.perf_counter()-start:.2f}s",
            "error:", (body.get("error") or "")[:60],
        )

    close = ds.close()
    if close is not None:
        await close


if __name__ == "__main__":
    asyncio.run(main())
