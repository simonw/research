"""Detailed test showing that restore(swap=True) destroys triggers for both libraries."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_swap_detail.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.row_factory = sqlite3.Row
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.commit()

sqlite_chronicle.enable_chronicle(db, "items")
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()

db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.execute("INSERT INTO items VALUES (2, 'Gadget', 19.99)")
db.commit()
db.execute("UPDATE items SET price = 14.99 WHERE id = 1")
db.commit()

triggers_before = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"Triggers BEFORE swap ({len(triggers_before)}):")
for t in triggers_before:
    print(f"  {dict(t)['name']}")

chronicle_ver_before = db.execute("SELECT MAX(__version) FROM _chronicle_items").fetchone()[0]
history_count_before = db.execute("SELECT COUNT(*) FROM _history_json_items").fetchone()[0]
print(f"\nChronicle max version before: {chronicle_ver_before}")
print(f"History-json entry count before: {history_count_before}")

print("\n=== Performing restore(swap=True) ===")
sqlite_history_json.restore(db, "items", up_to_id=2, swap=True)
db.commit()

triggers_after = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"\nTriggers AFTER swap ({len(triggers_after)}):")
for t in triggers_after:
    print(f"  {dict(t)['name']}")

if len(triggers_after) == 0:
    print("\n*** ALL TRIGGERS DESTROYED BY restore(swap=True) ***")

print("\n=== Attempting operations after swap (no triggers active) ===")
db.execute("INSERT INTO items VALUES (3, 'Post-swap', 99.99)")
db.commit()

chronicle_ver_after = db.execute("SELECT MAX(__version) FROM _chronicle_items").fetchone()[0]
history_count_after = db.execute("SELECT COUNT(*) FROM _history_json_items").fetchone()[0]
print(f"Chronicle max version after insert: {chronicle_ver_after} (was {chronicle_ver_before})")
print(f"History-json entry count after insert: {history_count_after} (was {history_count_before})")

if chronicle_ver_after == chronicle_ver_before:
    print("*** Chronicle DID NOT track the insert (triggers gone!) ***")
if history_count_after == history_count_before:
    print("*** History-json DID NOT track the insert (triggers gone!) ***")

print("\nis_chronicle_enabled() says:", sqlite_chronicle.is_chronicle_enabled(db, "items"))
print("(This is MISLEADING - it checks for the table, not triggers)")

print("\n=== Fix: Re-enable both after swap ===")
sqlite_chronicle.enable_chronicle(db, "items")
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()

triggers_fixed = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"Triggers after re-enable ({len(triggers_fixed)}):")
for t in triggers_fixed:
    print(f"  {dict(t)['name']}")

print("\nTest insert after re-enable:")
db.execute("INSERT INTO items VALUES (4, 'Post-fix', 199.99)")
db.commit()

chronicle_ver_fixed = db.execute("SELECT MAX(__version) FROM _chronicle_items").fetchone()[0]
history_count_fixed = db.execute("SELECT COUNT(*) FROM _history_json_items").fetchone()[0]
print(f"Chronicle max version: {chronicle_ver_fixed}")
print(f"History-json entry count: {history_count_fixed}")

if chronicle_ver_fixed > chronicle_ver_after:
    print("Chronicle is tracking again!")
if history_count_fixed > history_count_after:
    print("History-json is tracking again!")

db.close()
print("\nDetailed swap test completed!")
