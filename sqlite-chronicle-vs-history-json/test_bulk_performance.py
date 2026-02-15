"""Test bulk operations and relative overhead of having both libraries active."""
import sqlite3
import os
import time
import sqlite_chronicle
import sqlite_history_json

N = 1000

def timed(label, fn):
    start = time.perf_counter()
    fn()
    elapsed = (time.perf_counter() - start) * 1000
    print(f"  {label}: {elapsed:.1f}ms")
    return elapsed

def setup_db(path):
    if os.path.exists(path):
        os.remove(path)
    db = sqlite3.connect(path)
    db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
    db.commit()
    return db

print(f"Benchmark: {N} inserts, {N} updates, {N} deletes\n")

# Baseline: no tracking
db = setup_db("/tmp/bench_none.db")
def run_none():
    for i in range(N):
        db.execute("INSERT INTO items VALUES (?, ?, ?)", (i, f"item_{i}", i * 1.5))
    db.commit()
    for i in range(N):
        db.execute("UPDATE items SET price = ? WHERE id = ?", (i * 2.0, i))
    db.commit()
    for i in range(N):
        db.execute("DELETE FROM items WHERE id = ?", (i,))
    db.commit()
t_none = timed("No tracking", run_none)
db.close()

# Chronicle only
db = setup_db("/tmp/bench_chronicle.db")
sqlite_chronicle.enable_chronicle(db, "items")
db.commit()
def run_chronicle():
    for i in range(N):
        db.execute("INSERT INTO items VALUES (?, ?, ?)", (i, f"item_{i}", i * 1.5))
    db.commit()
    for i in range(N):
        db.execute("UPDATE items SET price = ? WHERE id = ?", (i * 2.0, i))
    db.commit()
    for i in range(N):
        db.execute("DELETE FROM items WHERE id = ?", (i,))
    db.commit()
t_chronicle = timed("Chronicle only", run_chronicle)
db.close()

# History-json only
db = setup_db("/tmp/bench_history.db")
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()
def run_history():
    for i in range(N):
        db.execute("INSERT INTO items VALUES (?, ?, ?)", (i, f"item_{i}", i * 1.5))
    db.commit()
    for i in range(N):
        db.execute("UPDATE items SET price = ? WHERE id = ?", (i * 2.0, i))
    db.commit()
    for i in range(N):
        db.execute("DELETE FROM items WHERE id = ?", (i,))
    db.commit()
t_history = timed("History-json only", run_history)
db.close()

# Both libraries
db = setup_db("/tmp/bench_both.db")
sqlite_chronicle.enable_chronicle(db, "items")
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()
def run_both():
    for i in range(N):
        db.execute("INSERT INTO items VALUES (?, ?, ?)", (i, f"item_{i}", i * 1.5))
    db.commit()
    for i in range(N):
        db.execute("UPDATE items SET price = ? WHERE id = ?", (i * 2.0, i))
    db.commit()
    for i in range(N):
        db.execute("DELETE FROM items WHERE id = ?", (i,))
    db.commit()
t_both = timed("Both libraries", run_both)
db.close()

print(f"\nOverhead vs baseline:")
print(f"  Chronicle only: {t_chronicle/t_none:.2f}x")
print(f"  History-json only: {t_history/t_none:.2f}x")
print(f"  Both libraries: {t_both/t_none:.2f}x")

# Verify data integrity for the "both" case
db = sqlite3.connect("/tmp/bench_both.db")
chronicle_count = db.execute("SELECT count(*) FROM _chronicle_items").fetchone()[0]
history_count = db.execute("SELECT count(*) FROM _history_json_items").fetchone()[0]
items_count = db.execute("SELECT count(*) FROM items").fetchone()[0]

print(f"\nData integrity (both libraries, {N} ops each):")
print(f"  Items remaining: {items_count} (expected: 0 after delete all)")
print(f"  Chronicle rows: {chronicle_count} (expected: {N} - one per PK)")
print(f"  History-json entries: {history_count} (expected: {N*3} - one per operation)")

# Check chronicle: all should be marked deleted
deleted = db.execute("SELECT count(*) FROM _chronicle_items WHERE __deleted=1").fetchone()[0]
print(f"  Chronicle deleted entries: {deleted} (expected: {N})")

db.close()
print("\nBenchmark completed!")
