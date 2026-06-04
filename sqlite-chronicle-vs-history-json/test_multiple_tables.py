"""Test both libraries tracking multiple tables simultaneously."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_multi_table.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)")
db.execute("CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER, total REAL)")
db.execute("CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
db.commit()

print("=== Enable both libraries on all three tables ===")
for table in ["users", "orders", "products"]:
    sqlite_chronicle.enable_chronicle(db, table)
    sqlite_history_json.enable_tracking(db, table, populate_table=False)
db.commit()

triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print(f"Total triggers: {len(triggers)}")
tables = db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").fetchall()
print(f"Total tables: {len(tables)}")
print("Tables:", [t[0] for t in tables])

print(f"\nChronicled tables: {sqlite_chronicle.list_chronicled_tables(db)}")

print("\n=== Cross-table operations ===")
db.execute("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com')")
db.execute("INSERT INTO products VALUES (100, 'Widget', 9.99)")
db.execute("INSERT INTO orders VALUES (1000, 1, 9.99)")
db.commit()

db.execute("UPDATE users SET email = 'alice@new.com' WHERE id = 1")
db.execute("UPDATE orders SET total = 19.98 WHERE id = 1000")
db.commit()

print("All three tables tracked successfully.")

print("\nChronicle - users:")
for row in db.execute("SELECT * FROM _chronicle_users").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_users").description]
    print(f"  {dict(zip(cols, row))}")

print("Chronicle - orders:")
for row in db.execute("SELECT * FROM _chronicle_orders").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_orders").description]
    print(f"  {dict(zip(cols, row))}")

print("Chronicle - products:")
for row in db.execute("SELECT * FROM _chronicle_products").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_products").description]
    print(f"  {dict(zip(cols, row))}")

print("\nHistory-json - users:")
for row in db.execute("SELECT * FROM _history_json_users ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_users").description]
    print(f"  {dict(zip(cols, row))}")

print("History-json - orders:")
for row in db.execute("SELECT * FROM _history_json_orders ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_orders").description]
    print(f"  {dict(zip(cols, row))}")

print("\n=== Change group spanning multiple tables ===")
with sqlite_history_json.change_group(db, note="Cross-table update") as gid:
    db.execute("INSERT INTO products VALUES (101, 'Gadget', 19.99)")
    db.execute("INSERT INTO orders VALUES (1001, 1, 19.99)")
    db.execute("UPDATE users SET name = 'Alice Smith' WHERE id = 1")
db.commit()

print(f"Change group {gid} applied across all tables.")
print("\nHistory-json users (latest):")
row = db.execute("SELECT * FROM _history_json_users ORDER BY id DESC LIMIT 1").fetchone()
cols = [d[0] for d in db.execute("SELECT * FROM _history_json_users").description]
print(f"  {dict(zip(cols, row))}")

print("History-json orders (latest):")
row = db.execute("SELECT * FROM _history_json_orders ORDER BY id DESC LIMIT 1").fetchone()
cols = [d[0] for d in db.execute("SELECT * FROM _history_json_orders").description]
print(f"  {dict(zip(cols, row))}")

print("History-json products (latest):")
row = db.execute("SELECT * FROM _history_json_products ORDER BY id DESC LIMIT 1").fetchone()
cols = [d[0] for d in db.execute("SELECT * FROM _history_json_products").description]
print(f"  {dict(zip(cols, row))}")

print("\n(All three share the same group id from _history_json table)")

# Check shared groups table
groups = db.execute("SELECT * FROM _history_json").fetchall()
print(f"Groups table: {len(groups)} groups")
for g in groups:
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json").description]
    print(f"  {dict(zip(cols, g))}")

db.close()
print("\nMultiple table tests completed successfully!")
