"""Test enabling chronicle FIRST, then history-json on the same table."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_both_chronicle_first.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.execute("INSERT INTO items VALUES (2, 'Gadget', 19.99)")
db.commit()

print("=== Step 1: Enable chronicle first ===")
sqlite_chronicle.enable_chronicle(db, "items")
db.commit()

print("=== Step 2: Enable history-json second ===")
sqlite_history_json.enable_tracking(db, "items")
db.commit()

# List all tables
tables = db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").fetchall()
print("\nAll tables:", [t[0] for t in tables])

# List all triggers
triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print("All triggers:", [t[0] for t in triggers])
print(f"Total trigger count: {len(triggers)}")

# Show initial chronicle state
print("\n=== Initial chronicle state ===")
for row in db.execute("SELECT * FROM _chronicle_items").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

# Show initial history state
print("\n=== Initial history-json state ===")
for row in db.execute("SELECT * FROM _history_json_items").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(dict(zip(cols, row)))

print("\n=== Step 3: INSERT a new row ===")
db.execute("INSERT INTO items VALUES (3, 'Doohickey', 29.99)")
db.commit()
print("Inserted row 3")

print("\nChronicle after INSERT:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nHistory-json after INSERT:")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(dict(zip(cols, row)))

print("\n=== Step 4: UPDATE a row ===")
db.execute("UPDATE items SET price = 12.99 WHERE id = 1")
db.commit()
print("Updated row 1 price to 12.99")

print("\nChronicle after UPDATE:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nHistory-json after UPDATE:")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(dict(zip(cols, row)))

print("\n=== Step 5: DELETE a row ===")
db.execute("DELETE FROM items WHERE id = 2")
db.commit()
print("Deleted row 2")

print("\nChronicle after DELETE:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(dict(zip(cols, row)))

print("\nHistory-json after DELETE:")
for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
    print(dict(zip(cols, row)))

print("\n=== Step 6: Verify items table is correct ===")
print("Items table:")
for row in db.execute("SELECT * FROM items ORDER BY id").fetchall():
    print(row)

db.close()
