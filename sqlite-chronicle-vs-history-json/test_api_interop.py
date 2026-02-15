"""Test that the Python APIs of both libraries work correctly when both are active."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_api_interop.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.row_factory = sqlite3.Row
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.commit()

# Enable both
sqlite_chronicle.enable_chronicle(db, "items")
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
db.commit()

# Insert some data
db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.execute("INSERT INTO items VALUES (2, 'Gadget', 19.99)")
db.commit()

# Update
db.execute("UPDATE items SET price = 12.99 WHERE id = 1")
db.commit()

# Delete
db.execute("DELETE FROM items WHERE id = 2")
db.commit()

# Insert more
db.execute("INSERT INTO items VALUES (3, 'Thingamajig', 39.99)")
db.commit()

print("=" * 60)
print("TEST: sqlite_chronicle.updates_since()")
print("=" * 60)
print("\nAll changes (since=None):")
for change in sqlite_chronicle.updates_since(db, "items"):
    print(f"  pks={change.pks}, version={change.version}, deleted={change.deleted}, row={change.row}")

print("\nChanges since version 2:")
for change in sqlite_chronicle.updates_since(db, "items", since=2):
    print(f"  pks={change.pks}, version={change.version}, deleted={change.deleted}, row={change.row}")

print("\n" + "=" * 60)
print("TEST: sqlite_history_json.get_history()")
print("=" * 60)
history = sqlite_history_json.get_history(db, "items")
for entry in history:
    print(f"  id={entry['id']}, op={entry['operation']}, pk={entry['pk']}, values={entry['updated_values']}")

print("\n" + "=" * 60)
print("TEST: sqlite_history_json.get_row_history()")
print("=" * 60)
print("\nHistory for row id=1:")
row_hist = sqlite_history_json.get_row_history(db, "items", {"id": 1})
for entry in row_hist:
    print(f"  id={entry['id']}, op={entry['operation']}, values={entry['updated_values']}")

print("\n" + "=" * 60)
print("TEST: sqlite_history_json.restore()")
print("=" * 60)
# Get the history entry id for the point before the delete
all_hist = sqlite_history_json.get_history(db, "items")
# The entries are newest-first, so the delete is entry id 4, the update is id 3
# We want to restore to just after the update (id 3) - before the delete
print("\nRestoring to up_to_id=3 (before delete of row 2):")
restored_name = sqlite_history_json.restore(db, "items", up_to_id=3)
print(f"Restored table name: {restored_name}")

print("\nRestored table contents:")
for row in db.execute(f"SELECT * FROM [{restored_name}] ORDER BY id").fetchall():
    print(f"  {dict(row)}")

print("\nOriginal table still intact:")
for row in db.execute("SELECT * FROM items ORDER BY id").fetchall():
    print(f"  {dict(row)}")

# Clean up restored table
db.execute(f"DROP TABLE [{restored_name}]")
db.commit()

print("\n" + "=" * 60)
print("TEST: sqlite_chronicle.is_chronicle_enabled()")
print("=" * 60)
print(f"Chronicle enabled for 'items': {sqlite_chronicle.is_chronicle_enabled(db, 'items')}")
print(f"Chronicled tables: {sqlite_chronicle.list_chronicled_tables(db)}")

print("\n" + "=" * 60)
print("TEST: sqlite_history_json change_group()")
print("=" * 60)
with sqlite_history_json.change_group(db, note="Batch price update") as group_id:
    print(f"Group ID: {group_id}")
    db.execute("UPDATE items SET price = price * 1.1 WHERE id = 1")
    db.execute("INSERT INTO items VALUES (4, 'Whatsit', 49.99)")
db.commit()

print("\nHistory after change_group:")
recent = sqlite_history_json.get_history(db, "items", limit=3)
for entry in recent:
    print(f"  id={entry['id']}, op={entry['operation']}, pk={entry['pk']}, group={entry['group']}, group_note={entry.get('group_note')}")

print("\nChronicle after change_group:")
for change in sqlite_chronicle.updates_since(db, "items", since=4):
    print(f"  pks={change.pks}, version={change.version}, row={change.row}")

db.close()
print("\nAll API interop tests completed successfully!")
