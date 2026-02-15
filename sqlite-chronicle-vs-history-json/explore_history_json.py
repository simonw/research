"""Explore sqlite-history-json: tables, triggers, and behavior."""
import sqlite3
import os
import sqlite_history_json

DB = "/tmp/test_history_json.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.execute("INSERT INTO items VALUES (2, 'Gadget', 19.99)")
db.commit()

sqlite_history_json.enable_tracking(db, "items")
db.commit()

# Show what tables exist now
tables = db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").fetchall()
print("Tables:", [t[0] for t in tables])

# Show history table schema
schema = db.execute("SELECT sql FROM sqlite_master WHERE name='_history_json_items'").fetchone()
print("\nHistory JSON table schema:")
print(schema[0])

# Show groups table schema
schema2 = db.execute("SELECT sql FROM sqlite_master WHERE name='_history_json'").fetchone()
print("\nGroups table schema:")
print(schema2[0])

# Show triggers
triggers = db.execute("SELECT name, sql FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"\nTriggers ({len(triggers)}):")
for name, sql in triggers:
    print(f"\n--- {name} ---")
    print(sql)

# Show history table contents
print("\nHistory JSON table contents after initial setup:")
rows = db.execute("SELECT * FROM _history_json_items").fetchall()
cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
print("Columns:", cols)
for row in rows:
    print(dict(zip(cols, row)))

db.close()
