"""Test disabling one library while the other remains active."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_disable.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.commit()

sqlite_chronicle.enable_chronicle(db, "items")
sqlite_history_json.enable_tracking(db, "items")
db.commit()

triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"Initial triggers ({len(triggers)}):", [t[0] for t in triggers])

print("\n=== Disable chronicle, keep history-json active ===")
sqlite_chronicle.disable_chronicle(db, "items")
db.commit()

triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"Triggers after disabling chronicle ({len(triggers)}):", [t[0] for t in triggers])

print("\nPerforming operations with only history-json active:")
db.execute("INSERT INTO items VALUES (2, 'Gadget', 19.99)")
db.execute("UPDATE items SET price = 12.99 WHERE id = 1")
db.commit()

print("History-json still works:")
history = sqlite_history_json.get_history(db, "items")
for entry in history:
    print(f"  id={entry['id']}, op={entry['operation']}, pk={entry['pk']}, values={entry['updated_values']}")

# Check chronicle table is gone
tables = db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE '_chronicle%'").fetchall()
print(f"\nChronicle tables remaining: {[t[0] for t in tables]}")

print("\n=== Re-enable chronicle ===")
sqlite_chronicle.enable_chronicle(db, "items")
db.commit()

triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"Triggers after re-enabling chronicle ({len(triggers)}):", [t[0] for t in triggers])

print("\nChronicle state after re-enable (picks up current state):")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\n=== Now disable history-json, keep chronicle active ===")
sqlite_history_json.disable_tracking(db, "items")
db.commit()

triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"Triggers after disabling history-json ({len(triggers)}):", [t[0] for t in triggers])

print("\nPerforming operations with only chronicle active:")
db.execute("INSERT INTO items VALUES (3, 'Doohickey', 29.99)")
db.execute("DELETE FROM items WHERE id = 2")
db.commit()

print("Chronicle still works:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

# History table still exists (data preserved) but no new entries
tables = db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE '_history_json%'").fetchall()
print(f"\nHistory-json tables still present: {[t[0] for t in tables]}")
count = db.execute("SELECT count(*) FROM _history_json_items").fetchone()[0]
print(f"History-json entries (frozen, no new ones): {count}")

print("\n=== Re-enable history-json ===")
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()

print("Both active again. Insert a new row:")
db.execute("INSERT INTO items VALUES (4, 'Whatsit', 49.99)")
db.commit()

print("\nChronicle:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nHistory-json (latest 2):")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id DESC LIMIT 2").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(dict(zip(cols, row)))

db.close()
print("\nDisable/re-enable tests completed successfully!")
