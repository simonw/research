"""Test what happens when history-json's restore(swap=True) replaces the tracked table."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_restore_swap.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.row_factory = sqlite3.Row
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.commit()

sqlite_chronicle.enable_chronicle(db, "items")
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()

# Build up some history
db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.execute("INSERT INTO items VALUES (2, 'Gadget', 19.99)")
db.commit()
db.execute("UPDATE items SET price = 14.99 WHERE id = 1")
db.commit()
db.execute("DELETE FROM items WHERE id = 2")
db.commit()
db.execute("INSERT INTO items VALUES (3, 'Doohickey', 29.99)")
db.commit()

print("=== State before restore ===")
print("Items table:")
for row in db.execute("SELECT * FROM items ORDER BY id").fetchall():
    print(f"  {dict(row)}")

print("\nChronicle:")
for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
    print(f"  {dict(zip(cols, dict(row).values()))}")

print("\nTriggers before restore:")
triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"  {[dict(t)['name'] for t in triggers]}")

print("\n=== Restoring to up_to_id=3 (before delete) WITH swap=True ===")
try:
    restored = sqlite_history_json.restore(db, "items", up_to_id=3, swap=True)
    db.commit()
    print(f"Restore returned: {restored}")

    print("\nItems table after swap:")
    for row in db.execute("SELECT * FROM items ORDER BY id").fetchall():
        print(f"  {dict(row)}")

    print("\nTriggers after swap:")
    triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
    print(f"  {[dict(t)['name'] for t in triggers]}")

    print("\nIs chronicle still enabled?", sqlite_chronicle.is_chronicle_enabled(db, "items"))

    print("\nChronicle table after swap:")
    try:
        for row in db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall():
            cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
            print(f"  {dict(zip(cols, dict(row).values()))}")
    except Exception as e:
        print(f"  Error: {e}")

    # Test if operations still work after swap
    print("\n=== Testing operations after swap ===")
    db.execute("INSERT INTO items VALUES (4, 'Post-swap item', 99.99)")
    db.commit()
    print("Insert after swap succeeded")

    # Check if chronicle tracked it
    try:
        chronicle_rows = db.execute("SELECT * FROM _chronicle_items ORDER BY id").fetchall()
        print(f"Chronicle has {len(chronicle_rows)} rows")
        for row in chronicle_rows:
            cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
            print(f"  {dict(zip(cols, dict(row).values()))}")
    except Exception as e:
        print(f"Chronicle error: {e}")

    # Check if history tracked it
    latest = db.execute("SELECT * FROM _history_json_items ORDER BY id DESC LIMIT 1").fetchone()
    if latest:
        cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
        print(f"History-json latest: {dict(zip(cols, dict(latest).values()))}")

except Exception as e:
    print(f"Error during restore: {type(e).__name__}: {e}")
    import traceback
    traceback.print_exc()

db.close()
print("\nRestore swap test completed!")
