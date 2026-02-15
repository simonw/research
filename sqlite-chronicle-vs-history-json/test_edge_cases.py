"""Test edge cases when both libraries are active on the same table."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_edge_cases.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.commit()

# Enable both
sqlite_chronicle.enable_chronicle(db, "items")
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()

print("=" * 60)
print("EDGE CASE 1: INSERT OR REPLACE (upsert)")
print("=" * 60)
db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.commit()
print("After initial insert of row 1:")

db.execute("INSERT OR REPLACE INTO items VALUES (1, 'Widget Pro', 14.99)")
db.commit()
print("After INSERT OR REPLACE on same id=1:")

print("\nChronicle:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nHistory-json:")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(dict(zip(cols, row)))

print("\nItems table:")
for row in db.execute("SELECT * FROM items").fetchall():
    print(row)

print("\n" + "=" * 60)
print("EDGE CASE 2: INSERT OR REPLACE with no actual change")
print("=" * 60)
chronicle_before = db.execute("SELECT __version FROM _chronicle_items WHERE id=1").fetchone()
history_count_before = db.execute("SELECT count(*) FROM _history_json_items").fetchone()[0]

db.execute("INSERT OR REPLACE INTO items VALUES (1, 'Widget Pro', 14.99)")
db.commit()

chronicle_after = db.execute("SELECT __version FROM _chronicle_items WHERE id=1").fetchone()
history_count_after = db.execute("SELECT count(*) FROM _history_json_items").fetchone()[0]

print(f"Chronicle version before: {chronicle_before[0]}, after: {chronicle_after[0]}")
print(f"History entries before: {history_count_before}, after: {history_count_after}")
if chronicle_before[0] == chronicle_after[0]:
    print("Chronicle: NO version bump (detected no change) - SMART!")
else:
    print("Chronicle: Version bumped despite no actual change")

if history_count_before == history_count_after:
    print("History-json: No new entry (detected no change)")
else:
    print("History-json: New entry added despite no actual change")
    last = db.execute("SELECT * FROM _history_json_items ORDER BY id DESC LIMIT 1").fetchone()
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print("  Last entry:", dict(zip(cols, last)))

print("\n" + "=" * 60)
print("EDGE CASE 3: Delete then re-insert same primary key")
print("=" * 60)
db.execute("DELETE FROM items WHERE id = 1")
db.commit()
print("After deleting row 1:")

print("\nChronicle:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nNow re-inserting row 1 with new data:")
db.execute("INSERT INTO items VALUES (1, 'Widget Reborn', 99.99)")
db.commit()

print("\nChronicle after re-insert:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nHistory-json after re-insert:")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(dict(zip(cols, row)))

print("\n" + "=" * 60)
print("EDGE CASE 4: UPDATE with no actual change")
print("=" * 60)
chronicle_before = db.execute("SELECT __version FROM _chronicle_items WHERE id=1").fetchone()
history_count_before = db.execute("SELECT count(*) FROM _history_json_items").fetchone()[0]

db.execute("UPDATE items SET name='Widget Reborn' WHERE id=1")
db.commit()

chronicle_after = db.execute("SELECT __version FROM _chronicle_items WHERE id=1").fetchone()
history_count_after = db.execute("SELECT count(*) FROM _history_json_items").fetchone()[0]

print(f"Chronicle version before: {chronicle_before[0]}, after: {chronicle_after[0]}")
print(f"History entries before: {history_count_before}, after: {history_count_after}")
if chronicle_before[0] == chronicle_after[0]:
    print("Chronicle: NO version bump (WHEN clause filtered it)")
else:
    print("Chronicle: Version bumped despite no actual data change")

if history_count_before == history_count_after:
    print("History-json: No new entry")
else:
    print("History-json: New entry added")
    last = db.execute("SELECT * FROM _history_json_items ORDER BY id DESC LIMIT 1").fetchone()
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print("  Last entry:", dict(zip(cols, last)))

print("\n" + "=" * 60)
print("EDGE CASE 5: NULL values")
print("=" * 60)
db.execute("INSERT INTO items VALUES (10, NULL, NULL)")
db.commit()
print("After inserting row with NULLs:")

print("\nChronicle:")
for row in db.execute("SELECT * FROM _chronicle_items WHERE id=10").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nHistory-json (last entry):")
last = db.execute("SELECT * FROM _history_json_items ORDER BY id DESC LIMIT 1").fetchone()
cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
print(dict(zip(cols, last)))

db.execute("UPDATE items SET name='From Null' WHERE id=10")
db.commit()
print("\nAfter updating NULL name to 'From Null':")
last = db.execute("SELECT * FROM _history_json_items ORDER BY id DESC LIMIT 1").fetchone()
cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
print("History-json last entry:", dict(zip(cols, last)))

db.close()
print("\nAll edge case tests completed successfully!")
