"""Test enabling history-json FIRST, then chronicle on the same table."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_both_history_first.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.execute("INSERT INTO items VALUES (2, 'Gadget', 19.99)")
db.commit()

print("=== Step 1: Enable history-json FIRST ===")
sqlite_history_json.enable_tracking(db, "items")
db.commit()

print("=== Step 2: Enable chronicle SECOND ===")
sqlite_chronicle.enable_chronicle(db, "items")
db.commit()

# List all triggers
triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print("All triggers:", [t[0] for t in triggers])
print(f"Total trigger count: {len(triggers)}")

print("\n=== Step 3: INSERT a new row ===")
db.execute("INSERT INTO items VALUES (3, 'Doohickey', 29.99)")
db.commit()

print("Chronicle after INSERT:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nHistory-json after INSERT:")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(dict(zip(cols, row)))

print("\n=== Step 4: UPDATE a row ===")
db.execute("UPDATE items SET name = 'Super Widget', price = 15.99 WHERE id = 1")
db.commit()

print("Chronicle after UPDATE:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nHistory-json after UPDATE (should show both name and price changed):")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(dict(zip(cols, row)))

print("\n=== Step 5: DELETE a row ===")
db.execute("DELETE FROM items WHERE id = 2")
db.commit()

print("Chronicle after DELETE:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nHistory-json after DELETE:")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(dict(zip(cols, row)))

print("\n=== Verify items table ===")
for row in db.execute("SELECT * FROM items ORDER BY id").fetchall():
    print(row)

db.close()
print("\nAll operations completed successfully with history-json first, chronicle second.")
