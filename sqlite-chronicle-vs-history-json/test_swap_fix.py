"""Test the correct way to re-enable both after a swap."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_swap_fix.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.commit()

sqlite_chronicle.enable_chronicle(db, "items")
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()

db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.execute("INSERT INTO items VALUES (2, 'Gadget', 19.99)")
db.commit()

# Swap
sqlite_history_json.restore(db, "items", up_to_id=1, swap=True)
db.commit()

triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"After swap: {len(triggers)} triggers")

print("\n=== Approach 1: disable then re-enable chronicle ===")
print(f"is_chronicle_enabled: {sqlite_chronicle.is_chronicle_enabled(db, 'items')}")
# The issue is chronicle thinks it's already enabled because the table exists.
# We need to disable first, then re-enable.
sqlite_chronicle.disable_chronicle(db, "items")
db.commit()
print(f"After disable: is_chronicle_enabled = {sqlite_chronicle.is_chronicle_enabled(db, 'items')}")
sqlite_chronicle.enable_chronicle(db, "items")
db.commit()
print(f"After re-enable: is_chronicle_enabled = {sqlite_chronicle.is_chronicle_enabled(db, 'items')}")

# Re-enable history-json too
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()

triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"\nTriggers after proper fix ({len(triggers)}):")
for t in triggers:
    print(f"  {t[0]}")

print("\nTest operations:")
db.execute("INSERT INTO items VALUES (3, 'Test', 99.99)")
db.commit()

chronicle_ver = db.execute("SELECT MAX(__version) FROM _chronicle_items").fetchone()[0]
history_count = db.execute("SELECT COUNT(*) FROM _history_json_items").fetchone()[0]
print(f"Chronicle max version: {chronicle_ver}")
print(f"History-json entries: {history_count}")

print("\nChronicle rows:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(f"  {dict(zip(cols, row))}")

print("\nHistory-json latest:")
row = db.execute("SELECT * FROM _history_json_items ORDER BY id DESC LIMIT 1").fetchone()
cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
print(f"  {dict(zip(cols, row))}")

db.close()
print("\nProper fix after swap verified!")
