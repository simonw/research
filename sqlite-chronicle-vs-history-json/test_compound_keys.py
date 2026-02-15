"""Test both libraries with compound primary keys on the same table."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

DB = "/tmp/test_compound.db"
if os.path.exists(DB):
    os.remove(DB)

db = sqlite3.connect(DB)
db.execute("""
    CREATE TABLE user_roles (
        user_id INTEGER,
        role_id INTEGER,
        granted_at TEXT,
        PRIMARY KEY (user_id, role_id)
    )
""")
db.commit()

print("=== Enable both on compound PK table ===")
sqlite_chronicle.enable_chronicle(db, "user_roles")
sqlite_history_json.enable_tracking(db, "user_roles", populate_table=False)
db.commit()

# Show triggers
triggers = db.execute("SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name").fetchall()
print("Triggers:", [t[0] for t in triggers])

# Show table schemas
print("\nChronicle schema:")
print(db.execute("SELECT sql FROM sqlite_master WHERE name='_chronicle_user_roles'").fetchone()[0])
print("\nHistory-json schema:")
print(db.execute("SELECT sql FROM sqlite_master WHERE name='_history_json_user_roles'").fetchone()[0])

print("\n=== INSERT rows ===")
db.execute("INSERT INTO user_roles VALUES (1, 100, '2024-01-01')")
db.execute("INSERT INTO user_roles VALUES (1, 200, '2024-02-01')")
db.execute("INSERT INTO user_roles VALUES (2, 100, '2024-03-01')")
db.commit()

print("\nChronicle:")
for row in db.execute("SELECT * FROM _chronicle_user_roles").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_user_roles").description]
    print(dict(zip(cols, row)))

print("\nHistory-json:")
for row in db.execute("SELECT * FROM _history_json_user_roles ORDER BY id").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _history_json_user_roles").description]
    print(dict(zip(cols, row)))

print("\n=== UPDATE a row ===")
db.execute("UPDATE user_roles SET granted_at = '2024-06-15' WHERE user_id=1 AND role_id=100")
db.commit()

print("\nChronicle after update:")
for row in db.execute("SELECT * FROM _chronicle_user_roles").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_user_roles").description]
    print(dict(zip(cols, row)))

print("\nHistory-json last entry:")
last = db.execute("SELECT * FROM _history_json_user_roles ORDER BY id DESC LIMIT 1").fetchone()
cols = [d[0] for d in db.execute("SELECT * FROM _history_json_user_roles").description]
print(dict(zip(cols, last)))

print("\n=== DELETE a row ===")
db.execute("DELETE FROM user_roles WHERE user_id=1 AND role_id=200")
db.commit()

print("\nChronicle after delete:")
for row in db.execute("SELECT * FROM _chronicle_user_roles").fetchall():
    cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_user_roles").description]
    print(dict(zip(cols, row)))

print("\nHistory-json last entry:")
last = db.execute("SELECT * FROM _history_json_user_roles ORDER BY id DESC LIMIT 1").fetchone()
cols = [d[0] for d in db.execute("SELECT * FROM _history_json_user_roles").description]
print(dict(zip(cols, last)))

db.close()
print("\nCompound key tests completed successfully!")
