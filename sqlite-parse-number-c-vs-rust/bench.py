"""Benchmark parse_number() across all extensions, inside SQLite.

Usage: python bench.py [mixed|int|real]

Builds a 2M-row in-memory table, then times `select sum(parse_number(v))`
for each extension, interleaving runs. Reports min/median wall time and the
net per-row cost after subtracting a `sum(length(v))` baseline scan.
"""
import sqlite3, time, random, statistics, sys

KIND = sys.argv[1] if len(sys.argv) > 1 else "mixed"
N = 2_000_000
RUNS = 15
random.seed(1)

def gen():
    n = random.uniform(-1e6, 1e6)
    if KIND == "int":
        return f"${int(n):,}"
    if KIND == "real":
        return f"${n:,.2f}"
    r = random.random()
    if r < 0.25: return f"${n:,.2f}"
    if r < 0.45: return f"£{int(n):,}"
    if r < 0.60: return f"{n/1e4:.1f}%"
    if r < 0.80: return str(int(n))
    if r < 0.90: return f"{n:,.3f}"
    return random.choice(["n/a", "", "TBD", "-", "12abc"])

EXTS = {
    "c": "c/parse_number.so",
    "c-fast": "c-fast/parse_number.so",
    "rusqlite": "rust-rusqlite/target/release/libparse_number.so",
    "sqlite-loadable": "rust-loadable/target/release/libparse_number.so",
}
PURE_SQL = ("sum(cast(nullif(replace(replace(replace(replace(v, ',', ''), "
            "'$', ''), '£', ''), '%', ''), '') as real))")

src = sqlite3.connect(":memory:")
src.execute("create table t(v text)")
src.executemany("insert into t values (?)", ((gen(),) for _ in range(N)))
src.commit()

dbs = {}
for name, path in EXTS.items():
    db = sqlite3.connect(":memory:")
    src.backup(db)
    db.enable_load_extension(True)
    db.load_extension(path)
    dbs[name] = db

exprs = {"baseline length()": ("c", "sum(length(v))"),
         "pure SQL replace()": ("c", PURE_SQL)}
exprs.update({n: (n, "sum(parse_number(v))") for n in EXTS})
times = {k: [] for k in exprs}
for _ in range(RUNS):
    for k, (dbn, e) in exprs.items():
        t = time.perf_counter()
        dbs[dbn].execute(f"select {e} from t").fetchone()
        times[k].append(time.perf_counter() - t)

print(f"dataset={KIND} rows={N} runs={RUNS}")
base = min(times["baseline length()"])
for k, ts in times.items():
    m = min(ts)
    print(f"{k:20} min {m*1000:6.1f} ms  median {statistics.median(ts)*1000:6.1f} ms"
          f"  net {(m-base)/N*1e9:6.1f} ns/row")
