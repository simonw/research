"""Test INSERT OR REPLACE behavior with recursive_triggers enabled.

SQLite doc: "REPLACE conflict resolution strategy deletes pre-existing rows...
delete triggers fire if and only if recursive triggers are enabled."

By default recursive_triggers is OFF. Let's see what changes when it's ON."""
import sqlite3
import os
import sqlite_chronicle
import sqlite_history_json

def run_test(recursive_triggers):
    DB = f"/tmp/test_recursive_{recursive_triggers}.db"
    if os.path.exists(DB):
        os.remove(DB)

    db = sqlite3.connect(DB)
    if recursive_triggers:
        db.execute("PRAGMA recursive_triggers = ON")
    rt_val = db.execute("PRAGMA recursive_triggers").fetchone()[0]
    print(f"\n{'='*60}")
    print(f"recursive_triggers = {rt_val} ({'ON' if rt_val else 'OFF (default)'})")
    print(f"{'='*60}")

    db.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
    db.commit()

    sqlite_chronicle.enable_chronicle(db, "items")
    sqlite_history_json.enable_tracking(db, "items", populate_table=False)
    db.commit()

    db.execute("INSERT INTO items VALUES (1, 'Widget', 9.99)")
    db.commit()

    print("\nAfter initial insert:")
    print(f"  History entries: {db.execute('SELECT count(*) FROM _history_json_items').fetchone()[0]}")

    print("\nPerforming INSERT OR REPLACE with changed data...")
    try:
        db.execute("INSERT OR REPLACE INTO items VALUES (1, 'Widget Pro', 14.99)")
        db.commit()
        print("  Success!")
    except Exception as e:
        print(f"  ERROR: {e}")
        db.rollback()
        db.close()
        return

    print("\nHistory-json entries:")
    for row in db.execute("SELECT * FROM _history_json_items ORDER BY id").fetchall():
        cols = [d[0] for d in db.execute("SELECT * FROM _history_json_items").description]
        print(f"  {dict(zip(cols, row))}")

    ops = db.execute("SELECT operation FROM _history_json_items ORDER BY id").fetchall()
    ops_list = [r[0] for r in ops]
    print(f"\nOperations recorded: {ops_list}")

    delete_count = ops_list.count('delete')
    insert_count = ops_list.count('insert')
    print(f"  Deletes: {delete_count}, Inserts: {insert_count}")

    if delete_count > 0:
        print("  -> DELETE trigger FIRED during INSERT OR REPLACE")
    else:
        print("  -> DELETE trigger did NOT fire during INSERT OR REPLACE")

    print("\nChronicle state:")
    for row in db.execute("SELECT * FROM _chronicle_items").fetchall():
        cols = [d[0] for d in db.execute("SELECT * FROM _chronicle_items").description]
        print(f"  {dict(zip(cols, row))}")

    print("\nItems table:")
    for row in db.execute("SELECT * FROM items").fetchall():
        print(f"  {row}")

    db.close()

run_test(False)
run_test(True)

print("\n\n=== Summary ===")
print("With recursive_triggers OFF (default):")
print("  INSERT OR REPLACE -> history-json sees: insert, insert")
print("  DELETE trigger does NOT fire for the implicit delete")
print("  Chronicle handles this correctly via BEFORE INSERT snapshot")
print()
print("With recursive_triggers ON:")
print("  INSERT OR REPLACE -> history-json sees: insert, delete, insert")
print("  DELETE trigger FIRES for the implicit delete")
print("  Both libraries must handle the additional trigger execution")
