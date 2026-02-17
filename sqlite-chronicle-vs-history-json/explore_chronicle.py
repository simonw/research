"""Explore sqlite-chronicle: tables, triggers, and behavior."""
import sqlite3
import os
import sqlite_chronicle

DB = "/tmp/test_chronicle.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
db.execute("INSERT INTO items VALUES (2, 'Gadget', 19.99)")
db.commit()

sqlite_chronicle.enable_chronicle(db, "items")
db.commit()

# Show what tables exist now
tables = db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").fetchall()
print("Tables:", [t[0] for t in tables])

# Show chronicle table schema
schema = db.execute("SELECT sql FROM sqlite_master WHERE name='_chronicle_items'").fetchone()
print("\nChronicle table schema:")
print(schema[0])

# Show triggers
triggers = db.execute("SELECT name, sql FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"\nTriggers ({len(triggers)}):")
for name, sql in triggers:
    print(f"\n--- {name} ---")
    print(sql)

# Show chronicle table contents
print("\nChronicle table contents after initial setup:")
rows = db.execute("SELECT * FROM _chronicle_items").fetchall()
cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
print("Columns:", cols)
for row in rows:
    print(dict(zip(cols, row)))

db.close()
