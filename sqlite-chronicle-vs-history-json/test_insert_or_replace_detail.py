"""Detailed analysis of INSERT OR REPLACE handling by both libraries.

INSERT OR REPLACE in SQLite actually does DELETE + INSERT when there's a conflict.
This means both libraries' delete AND insert triggers fire. Let's see what happens."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_ior_detail.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.commit()

sqlite_chronicle.enable_chronicle(db, "items")
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()

print("=== Insert initial row ===")
db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.commit()

print("\nChronicle:")
for row in db.execute("SELECT * FROM _chronicle_items").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(f"  {dict(zip(cols, row))}")

print("History-json:")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(f"  {dict(zip(cols, row))}")

print("\n=== INSERT OR REPLACE with changed data ===")
db.execute("INSERT OR REPLACE INTO items VALUES (1, 'Widget Pro', 14.99)")
db.commit()

print("\nChronicle (should update version, not create duplicate):")
for row in db.execute("SELECT * FROM _chronicle_items").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(f"  {dict(zip(cols, row))}")

print("\nHistory-json (what does it record for a REPLACE?):")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(f"  {dict(zip(cols, row))}")

print("\n=== Key observation ===")
history_ops = db.execute(
    "SELECT operation FROM _history_json_items WHERE pk_id=1 ORDER BY id"
).fetchall()
print(f"History-json recorded operations for pk=1: {[r[0] for r in history_ops]}")
print("(INSERT OR REPLACE triggers DELETE then INSERT in SQLite)")

# Does the delete get recorded?
delete_count = db.execute(
    "SELECT count(*) FROM _history_json_items WHERE operation='delete'"
).fetchone()[0]
insert_count = db.execute(
    "SELECT count(*) FROM _history_json_items WHERE operation='insert'"
).fetchone()[0]
print(f"Delete entries: {delete_count}, Insert entries: {insert_count}")

print("\n=== INSERT OR REPLACE with identical data ===")
chronicle_before = db.execute("SELECT __version FROM _chronicle_items WHERE id=1").fetchone()[0]
history_before = db.execute("SELECT count(*) FROM _history_json_items").fetchone()[0]

db.execute("INSERT OR REPLACE INTO items VALUES (1, 'Widget Pro', 14.99)")
db.commit()

chronicle_after = db.execute("SELECT __version FROM _chronicle_items WHERE id=1").fetchone()[0]
history_after = db.execute("SELECT count(*) FROM _history_json_items").fetchone()[0]

print(f"Chronicle version: {chronicle_before} -> {chronicle_after}")
print(f"History-json entries: {history_before} -> {history_after}")

if chronicle_before == chronicle_after:
    print("Chronicle: SMART - detected no real change, no version bump")
else:
    print("Chronicle: Version bumped despite identical data")

if history_before == history_after:
    print("History-json: SMART - detected no real change")
else:
    new_entries = db.execute(
        f"SELECT * FROM _history_json_items WHERE id > {history_before} ORDER BY id"
    ).fetchall()
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(f"History-json: Recorded {len(new_entries)} new entries for identical data:")
    for row in new_entries:
        print(f"  {dict(zip(cols, row))}")

print("\n=== Verify table ===")
for row in db.execute("SELECT * FROM items").fetchall():
    print(f"  {row}")

db.close()
print("\nINSERT OR REPLACE analysis completed!")
