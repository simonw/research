# sqlite-chronicle and sqlite-history-json: Same Table Investigation

*2026-02-15T16:18:27Z by Showboat 0.5.0*

## Setup

We'll install both libraries and explore them individually first, then test them together on the same table.

```bash
pip install sqlite-chronicle sqlite-history-json 2>&1
```

```output
Collecting sqlite-chronicle
  Downloading sqlite_chronicle-0.6.1-py3-none-any.whl.metadata (13 kB)
Collecting sqlite-history-json
  Downloading sqlite_history_json-0.4-py3-none-any.whl.metadata (22 kB)
Downloading sqlite_chronicle-0.6.1-py3-none-any.whl (15 kB)
Downloading sqlite_history_json-0.4-py3-none-any.whl (24 kB)
Installing collected packages: sqlite-history-json, sqlite-chronicle
Successfully installed sqlite-chronicle-0.6.1 sqlite-history-json-0.4
WARNING: Running pip as the 'root' user can result in broken permissions and conflicting behaviour with the system package manager. It is recommended to use a virtual environment instead: https://pip.pypa.io/warnings/venv
```

## Part 1: Exploring sqlite-chronicle

Let's first see how sqlite-chronicle works by itself, examining the triggers and tables it creates.

```bash
pip install sqlite-chronicle sqlite-history-json 2>&1 | tail -3
```

```output
Requirement already satisfied: sqlite-chronicle in /usr/local/lib/python3.11/dist-packages (0.6.1)
Requirement already satisfied: sqlite-history-json in /usr/local/lib/python3.11/dist-packages (0.4)
WARNING: Running pip as the 'root' user can result in broken permissions and conflicting behaviour with the system package manager. It is recommended to use a virtual environment instead: https://pip.pypa.io/warnings/venv
```

```bash
/usr/local/bin/python3 -c 'import sqlite_chronicle; print("OK:", sqlite_chronicle)'
```

```output
OK: <module 'sqlite_chronicle' from '/usr/local/lib/python3.11/dist-packages/sqlite_chronicle.py'>
```

```bash
/usr/local/bin/python3 explore_chronicle.py
```

```output
Tables: ['_chronicle_items', '_chroniclesnapshots', 'items']

Chronicle table schema:
CREATE TABLE "_chronicle_items" (
  "id" INTEGER,
  __added_ms INTEGER,
  __updated_ms INTEGER,
  __version INTEGER,
  __deleted INTEGER DEFAULT 0,
  PRIMARY KEY("id")
)

Triggers (4):

--- chronicle_items_ad ---
CREATE TRIGGER "chronicle_items_ad"
AFTER DELETE ON "items"
FOR EACH ROW
WHEN NOT EXISTS(SELECT 1 FROM "_chroniclesnapshots" WHERE table_name = 'items' AND key = CAST(OLD."id" AS TEXT))
BEGIN
  UPDATE "_chronicle_items"
    SET __updated_ms = CAST((julianday('now') - 2440587.5)*86400*1000 AS INTEGER),
      __version = COALESCE((SELECT MAX(__version) FROM "_chronicle_items"),0) + 1,
      __deleted = 1
  WHERE "id"=OLD."id";
END

--- chronicle_items_ai ---
CREATE TRIGGER "chronicle_items_ai"
AFTER INSERT ON "items"
FOR EACH ROW
BEGIN
  -- Un-delete if re-inserting a previously deleted row
  UPDATE "_chronicle_items"
  SET __added_ms = CAST((julianday('now') - 2440587.5)*86400*1000 AS INTEGER), __updated_ms = CAST((julianday('now') - 2440587.5)*86400*1000 AS INTEGER), __version = COALESCE((SELECT MAX(__version) FROM "_chronicle_items"),0) + 1, __deleted = 0
  WHERE "id"=NEW."id" AND __deleted = 1;

  -- Replace with actual change: bump version
  UPDATE "_chronicle_items"
  SET __updated_ms = CAST((julianday('now') - 2440587.5)*86400*1000 AS INTEGER), __version = COALESCE((SELECT MAX(__version) FROM "_chronicle_items"),0) + 1
  WHERE "id"=NEW."id" AND __deleted = 0
    AND EXISTS(SELECT 1 FROM "_chroniclesnapshots" WHERE table_name = 'items' AND key = CAST(NEW."id" AS TEXT))
    AND json_array(quote(NEW."name"), quote(NEW."price")) IS NOT (SELECT value FROM "_chroniclesnapshots" WHERE table_name = 'items' AND key = CAST(NEW."id" AS TEXT));

  -- Clean up snapshot
  DELETE FROM "_chroniclesnapshots" WHERE table_name = 'items' AND key = CAST(NEW."id" AS TEXT);

  -- Fresh insert: create chronicle entry (NO INSERT OR IGNORE!)
  INSERT INTO "_chronicle_items"("id", __added_ms, __updated_ms, __version, __deleted)
  SELECT NEW."id", CAST((julianday('now') - 2440587.5)*86400*1000 AS INTEGER), CAST((julianday('now') - 2440587.5)*86400*1000 AS INTEGER), COALESCE((SELECT MAX(__version) FROM "_chronicle_items"),0) + 1, 0
  WHERE NOT EXISTS(SELECT 1 FROM "_chronicle_items" WHERE "id"=NEW."id");
END

--- chronicle_items_au ---
CREATE TRIGGER "chronicle_items_au"
AFTER UPDATE ON "items"
FOR EACH ROW
WHEN OLD."name" IS NOT NEW."name" OR OLD."price" IS NOT NEW."price"
BEGIN
  UPDATE "_chronicle_items"
  SET __updated_ms = CAST((julianday('now') - 2440587.5)*86400*1000 AS INTEGER),
    __version = COALESCE((SELECT MAX(__version) FROM "_chronicle_items"),0) + 1
  WHERE "id"=NEW."id";
END

--- chronicle_items_bi ---
CREATE TRIGGER "chronicle_items_bi"
BEFORE INSERT ON "items"
FOR EACH ROW
WHEN EXISTS(SELECT 1 FROM "items" WHERE "id"=NEW."id")
BEGIN
  INSERT OR REPLACE INTO "_chroniclesnapshots"(table_name, key, value)
  VALUES('items', CAST(NEW."id" AS TEXT), (SELECT json_array(quote("name"), quote("price")) FROM "items" WHERE "id"=NEW."id"));
END

Chronicle table contents after initial setup:
Columns: ['id', '__added_ms', '__updated_ms', '__version', '__deleted']
{'id': 1, '__added_ms': 1771172471489, '__updated_ms': 1771172471489, '__version': 1, '__deleted': 0}
{'id': 2, '__added_ms': 1771172471489, '__updated_ms': 1771172471489, '__version': 2, '__deleted': 0}
```

## Part 2: Exploring sqlite-history-json

Now let's see what sqlite-history-json creates - its audit tables, triggers, and initial state.

```bash
/usr/local/bin/python3 explore_history_json.py
```

```output
Tables: ['_history_json', '_history_json_items', 'items']

History JSON table schema:
CREATE TABLE [_history_json_items] (
    id integer primary key,
    timestamp text,
    operation text,
    [pk_id] INTEGER,
    updated_values text,
    [group] integer references [_history_json](id)
)

Groups table schema:
CREATE TABLE [_history_json] (
    id integer primary key,
    note text,
    current integer
)

Triggers (3):

--- _history_json_items_delete ---
CREATE TRIGGER [_history_json_items_delete]
after delete on [items]
begin
    insert into [_history_json_items] (timestamp, operation, [pk_id], updated_values, [group])
    values (
        strftime('%Y-%m-%d %H:%M:%f', 'now'),
        'delete',
        OLD.[id],
        null,
        (select id from [_history_json] where current = 1)
    );
end

--- _history_json_items_insert ---
CREATE TRIGGER [_history_json_items_insert]
after insert on [items]
begin
    insert into [_history_json_items] (timestamp, operation, [pk_id], updated_values, [group])
    values (
        strftime('%Y-%m-%d %H:%M:%f', 'now'),
        'insert',
        NEW.[id],
        json_object('name', case when NEW.[name] is null then json_object('null', 1) else NEW.[name] end, 'price', case when NEW.[price] is null then json_object('null', 1) else NEW.[price] end),
        (select id from [_history_json] where current = 1)
    );
end

--- _history_json_items_update ---
CREATE TRIGGER [_history_json_items_update]
after update on [items]
begin
    insert into [_history_json_items] (timestamp, operation, [pk_id], updated_values, [group])
    values (
        strftime('%Y-%m-%d %H:%M:%f', 'now'),
        'update',
        NEW.[id],
        json_patch(
            json_patch(
            '{}',
            case
                when OLD.[name] is not NEW.[name] then
                    case
                        when NEW.[name] is null then json_object('name', json_object('null', 1))
                        else json_object('name', NEW.[name])
                    end
                else '{}'
            end
        ),
            case
                when OLD.[price] is not NEW.[price] then
                    case
                        when NEW.[price] is null then json_object('price', json_object('null', 1))
                        else json_object('price', NEW.[price])
                    end
                else '{}'
            end
        ),
        (select id from [_history_json] where current = 1)
    );
end

History JSON table contents after initial setup:
Columns: ['id', 'timestamp', 'operation', 'pk_id', 'updated_values', 'group']
{'id': 1, 'timestamp': '2026-02-15 16:21:36.110', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name": "Widget", "price": 9.99}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:21:36.110', 'operation': 'insert', 'pk_id': 2, 'updated_values': '{"name": "Gadget", "price": 19.99}', 'group': None}
```

## Part 3: Both Libraries on the Same Table (Chronicle First)

This is the core test. We enable sqlite-chronicle first, then sqlite-history-json, then perform INSERT, UPDATE, and DELETE operations to see if both tracking systems work correctly.

```bash
/usr/local/bin/python3 test_both_chronicle_first.py
```

```output
=== Step 1: Enable chronicle first ===
=== Step 2: Enable history-json second ===

All tables: ['_chronicle_items', '_chroniclesnapshots', '_history_json', '_history_json_items', 'items']
All triggers: ['_history_json_items_delete', '_history_json_items_insert', '_history_json_items_update', 'chronicle_items_ad', 'chronicle_items_ai', 'chronicle_items_au', 'chronicle_items_bi']
Total trigger count: 7

=== Initial chronicle state ===
{'id': 1, '__added_ms': 1771172532458, '__updated_ms': 1771172532458, '__version': 1, '__deleted': 0}
{'id': 2, '__added_ms': 1771172532458, '__updated_ms': 1771172532458, '__version': 2, '__deleted': 0}

=== Initial history-json state ===
{'id': 1, 'timestamp': '2026-02-15 16:22:12.467', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name": "Widget", "price": 9.99}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:22:12.467', 'operation': 'insert', 'pk_id': 2, 'updated_values': '{"name": "Gadget", "price": 19.99}', 'group': None}

=== Step 3: INSERT a new row ===
Inserted row 3

Chronicle after INSERT:
{'id': 1, '__added_ms': 1771172532458, '__updated_ms': 1771172532458, '__version': 1, '__deleted': 0}
{'id': 2, '__added_ms': 1771172532458, '__updated_ms': 1771172532458, '__version': 2, '__deleted': 0}
{'id': 3, '__added_ms': 1771172532473, '__updated_ms': 1771172532473, '__version': 3, '__deleted': 0}

History-json after INSERT:
{'id': 1, 'timestamp': '2026-02-15 16:22:12.467', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name": "Widget", "price": 9.99}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:22:12.467', 'operation': 'insert', 'pk_id': 2, 'updated_values': '{"name": "Gadget", "price": 19.99}', 'group': None}
{'id': 3, 'timestamp': '2026-02-15 16:22:12.474', 'operation': 'insert', 'pk_id': 3, 'updated_values': '{"name":"Doohickey","price":29.99}', 'group': None}

=== Step 4: UPDATE a row ===
Updated row 1 price to 12.99

Chronicle after UPDATE:
{'id': 1, '__added_ms': 1771172532458, '__updated_ms': 1771172532484, '__version': 4, '__deleted': 0}
{'id': 2, '__added_ms': 1771172532458, '__updated_ms': 1771172532458, '__version': 2, '__deleted': 0}
{'id': 3, '__added_ms': 1771172532473, '__updated_ms': 1771172532473, '__version': 3, '__deleted': 0}

History-json after UPDATE:
{'id': 1, 'timestamp': '2026-02-15 16:22:12.467', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name": "Widget", "price": 9.99}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:22:12.467', 'operation': 'insert', 'pk_id': 2, 'updated_values': '{"name": "Gadget", "price": 19.99}', 'group': None}
{'id': 3, 'timestamp': '2026-02-15 16:22:12.474', 'operation': 'insert', 'pk_id': 3, 'updated_values': '{"name":"Doohickey","price":29.99}', 'group': None}
{'id': 4, 'timestamp': '2026-02-15 16:22:12.484', 'operation': 'update', 'pk_id': 1, 'updated_values': '{"price":12.99}', 'group': None}

=== Step 5: DELETE a row ===
Deleted row 2

Chronicle after DELETE:
{'id': 1, '__added_ms': 1771172532458, '__updated_ms': 1771172532484, '__version': 4, '__deleted': 0}
{'id': 2, '__added_ms': 1771172532458, '__updated_ms': 1771172532494, '__version': 5, '__deleted': 1}
{'id': 3, '__added_ms': 1771172532473, '__updated_ms': 1771172532473, '__version': 3, '__deleted': 0}

History-json after DELETE:
{'id': 1, 'timestamp': '2026-02-15 16:22:12.467', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name": "Widget", "price": 9.99}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:22:12.467', 'operation': 'insert', 'pk_id': 2, 'updated_values': '{"name": "Gadget", "price": 19.99}', 'group': None}
{'id': 3, 'timestamp': '2026-02-15 16:22:12.474', 'operation': 'insert', 'pk_id': 3, 'updated_values': '{"name":"Doohickey","price":29.99}', 'group': None}
{'id': 4, 'timestamp': '2026-02-15 16:22:12.484', 'operation': 'update', 'pk_id': 1, 'updated_values': '{"price":12.99}', 'group': None}
{'id': 5, 'timestamp': '2026-02-15 16:22:12.495', 'operation': 'delete', 'pk_id': 2, 'updated_values': None, 'group': None}

=== Step 6: Verify items table is correct ===
Items table:
(1, 'Widget', 12.99)
(3, 'Doohickey', 29.99)
```

## Part 4: Both Libraries on the Same Table (History-JSON First)

Now we reverse the order: enable sqlite-history-json first, then sqlite-chronicle. This tests whether enable-order matters.

```bash
/usr/local/bin/python3 test_both_history_first.py
```

```output
=== Step 1: Enable history-json FIRST ===
=== Step 2: Enable chronicle SECOND ===
All triggers: ['_history_json_items_delete', '_history_json_items_insert', '_history_json_items_update', 'chronicle_items_ad', 'chronicle_items_ai', 'chronicle_items_au', 'chronicle_items_bi']
Total trigger count: 7

=== Step 3: INSERT a new row ===
Chronicle after INSERT:
{'id': 1, '__added_ms': 1771172560263, '__updated_ms': 1771172560263, '__version': 1, '__deleted': 0}
{'id': 2, '__added_ms': 1771172560263, '__updated_ms': 1771172560263, '__version': 2, '__deleted': 0}
{'id': 3, '__added_ms': 1771172560271, '__updated_ms': 1771172560271, '__version': 3, '__deleted': 0}

History-json after INSERT:
{'id': 1, 'timestamp': '2026-02-15 16:22:40.241', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name": "Widget", "price": 9.99}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:22:40.241', 'operation': 'insert', 'pk_id': 2, 'updated_values': '{"name": "Gadget", "price": 19.99}', 'group': None}
{'id': 3, 'timestamp': '2026-02-15 16:22:40.272', 'operation': 'insert', 'pk_id': 3, 'updated_values': '{"name":"Doohickey","price":29.99}', 'group': None}

=== Step 4: UPDATE a row ===
Chronicle after UPDATE:
{'id': 1, '__added_ms': 1771172560263, '__updated_ms': 1771172560282, '__version': 4, '__deleted': 0}
{'id': 2, '__added_ms': 1771172560263, '__updated_ms': 1771172560263, '__version': 2, '__deleted': 0}
{'id': 3, '__added_ms': 1771172560271, '__updated_ms': 1771172560271, '__version': 3, '__deleted': 0}

History-json after UPDATE (should show both name and price changed):
{'id': 1, 'timestamp': '2026-02-15 16:22:40.241', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name": "Widget", "price": 9.99}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:22:40.241', 'operation': 'insert', 'pk_id': 2, 'updated_values': '{"name": "Gadget", "price": 19.99}', 'group': None}
{'id': 3, 'timestamp': '2026-02-15 16:22:40.272', 'operation': 'insert', 'pk_id': 3, 'updated_values': '{"name":"Doohickey","price":29.99}', 'group': None}
{'id': 4, 'timestamp': '2026-02-15 16:22:40.282', 'operation': 'update', 'pk_id': 1, 'updated_values': '{"name":"Super Widget","price":15.99}', 'group': None}

=== Step 5: DELETE a row ===
Chronicle after DELETE:
{'id': 1, '__added_ms': 1771172560263, '__updated_ms': 1771172560282, '__version': 4, '__deleted': 0}
{'id': 2, '__added_ms': 1771172560263, '__updated_ms': 1771172560290, '__version': 5, '__deleted': 1}
{'id': 3, '__added_ms': 1771172560271, '__updated_ms': 1771172560271, '__version': 3, '__deleted': 0}

History-json after DELETE:
{'id': 1, 'timestamp': '2026-02-15 16:22:40.241', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name": "Widget", "price": 9.99}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:22:40.241', 'operation': 'insert', 'pk_id': 2, 'updated_values': '{"name": "Gadget", "price": 19.99}', 'group': None}
{'id': 3, 'timestamp': '2026-02-15 16:22:40.272', 'operation': 'insert', 'pk_id': 3, 'updated_values': '{"name":"Doohickey","price":29.99}', 'group': None}
{'id': 4, 'timestamp': '2026-02-15 16:22:40.282', 'operation': 'update', 'pk_id': 1, 'updated_values': '{"name":"Super Widget","price":15.99}', 'group': None}
{'id': 5, 'timestamp': '2026-02-15 16:22:40.290', 'operation': 'delete', 'pk_id': 2, 'updated_values': None, 'group': None}

=== Verify items table ===
(1, 'Super Widget', 15.99)
(3, 'Doohickey', 29.99)

All operations completed successfully with history-json first, chronicle second.
```

## Part 5: Edge Cases

Testing tricky scenarios: INSERT OR REPLACE, no-op updates, delete-then-reinsert, and NULL values.

```bash
/usr/local/bin/python3 test_edge_cases.py
```

```output
============================================================
EDGE CASE 1: INSERT OR REPLACE (upsert)
============================================================
After initial insert of row 1:
After INSERT OR REPLACE on same id=1:

Chronicle:
{'id': 1, '__added_ms': 1771172606885, '__updated_ms': 1771172606892, '__version': 2, '__deleted': 0}

History-json:
{'id': 1, 'timestamp': '2026-02-15 16:23:26.885', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget","price":9.99}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:23:26.892', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget Pro","price":14.99}', 'group': None}

Items table:
(1, 'Widget Pro', 14.99)

============================================================
EDGE CASE 2: INSERT OR REPLACE with no actual change
============================================================
Chronicle version before: 2, after: 2
History entries before: 2, after: 3
Chronicle: NO version bump (detected no change) - SMART!
History-json: New entry added despite no actual change
  Last entry: {'id': 3, 'timestamp': '2026-02-15 16:23:26.900', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget Pro","price":14.99}', 'group': None}

============================================================
EDGE CASE 3: Delete then re-insert same primary key
============================================================
After deleting row 1:

Chronicle:
{'id': 1, '__added_ms': 1771172606885, '__updated_ms': 1771172606906, '__version': 3, '__deleted': 1}

Now re-inserting row 1 with new data:

Chronicle after re-insert:
{'id': 1, '__added_ms': 1771172606913, '__updated_ms': 1771172606913, '__version': 4, '__deleted': 0}

History-json after re-insert:
{'id': 1, 'timestamp': '2026-02-15 16:23:26.885', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget","price":9.99}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:23:26.892', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget Pro","price":14.99}', 'group': None}
{'id': 3, 'timestamp': '2026-02-15 16:23:26.900', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget Pro","price":14.99}', 'group': None}
{'id': 4, 'timestamp': '2026-02-15 16:23:26.906', 'operation': 'delete', 'pk_id': 1, 'updated_values': None, 'group': None}
{'id': 5, 'timestamp': '2026-02-15 16:23:26.914', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget Reborn","price":99.99}', 'group': None}

============================================================
EDGE CASE 4: UPDATE with no actual change
============================================================
Chronicle version before: 4, after: 4
History entries before: 5, after: 6
Chronicle: NO version bump (WHEN clause filtered it)
History-json: New entry added
  Last entry: {'id': 6, 'timestamp': '2026-02-15 16:23:26.922', 'operation': 'update', 'pk_id': 1, 'updated_values': '{}', 'group': None}

============================================================
EDGE CASE 5: NULL values
============================================================
After inserting row with NULLs:

Chronicle:
{'id': 10, '__added_ms': 1771172606928, '__updated_ms': 1771172606928, '__version': 5, '__deleted': 0}

History-json (last entry):
{'id': 7, 'timestamp': '2026-02-15 16:23:26.929', 'operation': 'insert', 'pk_id': 10, 'updated_values': '{"name":{"null":1},"price":{"null":1}}', 'group': None}

After updating NULL name to 'From Null':
History-json last entry: {'id': 8, 'timestamp': '2026-02-15 16:23:26.937', 'operation': 'update', 'pk_id': 10, 'updated_values': '{"name":"From Null"}', 'group': None}

All edge case tests completed successfully!
```

## Part 6: Compound Primary Keys

Testing both libraries with a table that has a compound primary key (user_id, role_id).

```bash
/usr/local/bin/python3 test_compound_keys.py
```

```output
=== Enable both on compound PK table ===
Triggers: ['_history_json_user_roles_delete', '_history_json_user_roles_insert', '_history_json_user_roles_update', 'chronicle_user_roles_ad', 'chronicle_user_roles_ai', 'chronicle_user_roles_au', 'chronicle_user_roles_bi']

Chronicle schema:
CREATE TABLE "_chronicle_user_roles" (
  "user_id" INTEGER, "role_id" INTEGER,
  __added_ms INTEGER,
  __updated_ms INTEGER,
  __version INTEGER,
  __deleted INTEGER DEFAULT 0,
  PRIMARY KEY("user_id", "role_id")
)

History-json schema:
CREATE TABLE [_history_json_user_roles] (
    id integer primary key,
    timestamp text,
    operation text,
    [pk_user_id] INTEGER, [pk_role_id] INTEGER,
    updated_values text,
    [group] integer references [_history_json](id)
)

=== INSERT rows ===

Chronicle:
{'user_id': 1, 'role_id': 100, '__added_ms': 1771172659319, '__updated_ms': 1771172659319, '__version': 1, '__deleted': 0}
{'user_id': 1, 'role_id': 200, '__added_ms': 1771172659320, '__updated_ms': 1771172659320, '__version': 2, '__deleted': 0}
{'user_id': 2, 'role_id': 100, '__added_ms': 1771172659320, '__updated_ms': 1771172659320, '__version': 3, '__deleted': 0}

History-json:
{'id': 1, 'timestamp': '2026-02-15 16:24:19.319', 'operation': 'insert', 'pk_user_id': 1, 'pk_role_id': 100, 'updated_values': '{"granted_at":"2024-01-01"}', 'group': None}
{'id': 2, 'timestamp': '2026-02-15 16:24:19.320', 'operation': 'insert', 'pk_user_id': 1, 'pk_role_id': 200, 'updated_values': '{"granted_at":"2024-02-01"}', 'group': None}
{'id': 3, 'timestamp': '2026-02-15 16:24:19.320', 'operation': 'insert', 'pk_user_id': 2, 'pk_role_id': 100, 'updated_values': '{"granted_at":"2024-03-01"}', 'group': None}

=== UPDATE a row ===

Chronicle after update:
{'user_id': 1, 'role_id': 100, '__added_ms': 1771172659319, '__updated_ms': 1771172659328, '__version': 4, '__deleted': 0}
{'user_id': 1, 'role_id': 200, '__added_ms': 1771172659320, '__updated_ms': 1771172659320, '__version': 2, '__deleted': 0}
{'user_id': 2, 'role_id': 100, '__added_ms': 1771172659320, '__updated_ms': 1771172659320, '__version': 3, '__deleted': 0}

History-json last entry:
{'id': 4, 'timestamp': '2026-02-15 16:24:19.329', 'operation': 'update', 'pk_user_id': 1, 'pk_role_id': 100, 'updated_values': '{"granted_at":"2024-06-15"}', 'group': None}

=== DELETE a row ===

Chronicle after delete:
{'user_id': 1, 'role_id': 100, '__added_ms': 1771172659319, '__updated_ms': 1771172659328, '__version': 4, '__deleted': 0}
{'user_id': 1, 'role_id': 200, '__added_ms': 1771172659320, '__updated_ms': 1771172659340, '__version': 5, '__deleted': 1}
{'user_id': 2, 'role_id': 100, '__added_ms': 1771172659320, '__updated_ms': 1771172659320, '__version': 3, '__deleted': 0}

History-json last entry:
{'id': 5, 'timestamp': '2026-02-15 16:24:19.340', 'operation': 'delete', 'pk_user_id': 1, 'pk_role_id': 200, 'updated_values': None, 'group': None}

Compound key tests completed successfully!
```

## Part 7: Python API Interoperability

Testing that the Python-level APIs (updates_since, get_history, restore, change_group) all work correctly when both libraries are active.

```bash
/usr/local/bin/python3 test_api_interop.py
```

```output
============================================================
TEST: sqlite_chronicle.updates_since()
============================================================

All changes (since=None):
  pks=(1,), version=3, deleted=False, row={'id': 1, 'name': 'Widget', 'price': 12.99}
  pks=(2,), version=4, deleted=True, row={'id': 2, 'name': None, 'price': None}
  pks=(3,), version=5, deleted=False, row={'id': 3, 'name': 'Thingamajig', 'price': 39.99}

Changes since version 2:
  pks=(1,), version=3, deleted=False, row={'id': 1, 'name': 'Widget', 'price': 12.99}
  pks=(2,), version=4, deleted=True, row={'id': 2, 'name': None, 'price': None}
  pks=(3,), version=5, deleted=False, row={'id': 3, 'name': 'Thingamajig', 'price': 39.99}

============================================================
TEST: sqlite_history_json.get_history()
============================================================
  id=5, op=insert, pk={'id': 3}, values={'name': 'Thingamajig', 'price': 39.99}
  id=4, op=delete, pk={'id': 2}, values=None
  id=3, op=update, pk={'id': 1}, values={'price': 12.99}
  id=2, op=insert, pk={'id': 2}, values={'name': 'Gadget', 'price': 19.99}
  id=1, op=insert, pk={'id': 1}, values={'name': 'Widget', 'price': 9.99}

============================================================
TEST: sqlite_history_json.get_row_history()
============================================================

History for row id=1:
  id=3, op=update, values={'price': 12.99}
  id=1, op=insert, values={'name': 'Widget', 'price': 9.99}

============================================================
TEST: sqlite_history_json.restore()
============================================================

Restoring to up_to_id=3 (before delete of row 2):
Restored table name: items_restored

Restored table contents:
  {'id': 1, 'name': 'Widget', 'price': 12.99}
  {'id': 2, 'name': 'Gadget', 'price': 19.99}

Original table still intact:
  {'id': 1, 'name': 'Widget', 'price': 12.99}
  {'id': 3, 'name': 'Thingamajig', 'price': 39.99}

============================================================
TEST: sqlite_chronicle.is_chronicle_enabled()
============================================================
Chronicle enabled for 'items': True
Chronicled tables: ['items']

============================================================
TEST: sqlite_history_json change_group()
============================================================
Group ID: 1

History after change_group:
  id=7, op=insert, pk={'id': 4}, group=1, group_note=Batch price update
  id=6, op=update, pk={'id': 1}, group=1, group_note=Batch price update
  id=5, op=insert, pk={'id': 3}, group=None, group_note=None

Chronicle after change_group:
  pks=(3,), version=5, row={'id': 3, 'name': 'Thingamajig', 'price': 39.99}
  pks=(1,), version=6, row={'id': 1, 'name': 'Widget', 'price': 14.289000000000001}
  pks=(4,), version=7, row={'id': 4, 'name': 'Whatsit', 'price': 49.99}

All API interop tests completed successfully!
```

## Part 8: Disable/Re-enable One Library

What happens if you disable chronicle while history-json is still active, and vice versa? Does the remaining library continue working?

```bash
/usr/local/bin/python3 test_disable_reenable.py
```

```output
Initial triggers (7): ['_history_json_items_delete', '_history_json_items_insert', '_history_json_items_update', 'chronicle_items_ad', 'chronicle_items_ai', 'chronicle_items_au', 'chronicle_items_bi']

=== Disable chronicle, keep history-json active ===
Triggers after disabling chronicle (3): ['_history_json_items_delete', '_history_json_items_insert', '_history_json_items_update']

Performing operations with only history-json active:
History-json still works:
  id=3, op=update, pk={'id': 1}, values={'price': 12.99}
  id=2, op=insert, pk={'id': 2}, values={'name': 'Gadget', 'price': 19.99}
  id=1, op=insert, pk={'id': 1}, values={'name': 'Widget', 'price': 9.99}

Chronicle tables remaining: ['_chroniclesnapshots']

=== Re-enable chronicle ===
Triggers after re-enabling chronicle (7): ['_history_json_items_delete', '_history_json_items_insert', '_history_json_items_update', 'chronicle_items_ad', 'chronicle_items_ai', 'chronicle_items_au', 'chronicle_items_bi']

Chronicle state after re-enable (picks up current state):
{'id': 1, '__added_ms': 1771172729241, '__updated_ms': 1771172729241, '__version': 1, '__deleted': 0}
{'id': 2, '__added_ms': 1771172729241, '__updated_ms': 1771172729241, '__version': 2, '__deleted': 0}

=== Now disable history-json, keep chronicle active ===
Triggers after disabling history-json (4): ['chronicle_items_ad', 'chronicle_items_ai', 'chronicle_items_au', 'chronicle_items_bi']

Performing operations with only chronicle active:
Chronicle still works:
{'id': 1, '__added_ms': 1771172729241, '__updated_ms': 1771172729241, '__version': 1, '__deleted': 0}
{'id': 2, '__added_ms': 1771172729241, '__updated_ms': 1771172729255, '__version': 4, '__deleted': 1}
{'id': 3, '__added_ms': 1771172729255, '__updated_ms': 1771172729255, '__version': 3, '__deleted': 0}

History-json tables still present: ['_history_json', '_history_json_items']
History-json entries (frozen, no new ones): 3

=== Re-enable history-json ===
Both active again. Insert a new row:

Chronicle:
{'id': 1, '__added_ms': 1771172729241, '__updated_ms': 1771172729241, '__version': 1, '__deleted': 0}
{'id': 2, '__added_ms': 1771172729241, '__updated_ms': 1771172729255, '__version': 4, '__deleted': 1}
{'id': 3, '__added_ms': 1771172729255, '__updated_ms': 1771172729255, '__version': 3, '__deleted': 0}
{'id': 4, '__added_ms': 1771172729268, '__updated_ms': 1771172729268, '__version': 5, '__deleted': 0}

History-json (latest 2):
{'id': 4, 'timestamp': '2026-02-15 16:25:29.268', 'operation': 'insert', 'pk_id': 4, 'updated_values': '{"name":"Whatsit","price":49.99}', 'group': None}
{'id': 3, 'timestamp': '2026-02-15 16:25:29.223', 'operation': 'update', 'pk_id': 1, 'updated_values': '{"price":12.99}', 'group': None}

Disable/re-enable tests completed successfully!
```

## Part 9: restore(swap=True) Interaction

The most interesting edge case: what happens when history-json's restore(swap=True) atomically replaces the original table? Does this break chronicle's triggers?

```bash
/usr/local/bin/python3 test_restore_swap.py
```

```output
=== State before restore ===
Items table:
  {'id': 1, 'name': 'Widget', 'price': 14.99}
  {'id': 3, 'name': 'Doohickey', 'price': 29.99}

Chronicle:
  {'id': 1, '__added_ms': 1771172745157, '__updated_ms': 1771172745165, '__version': 3, '__deleted': 0}
  {'id': 2, '__added_ms': 1771172745158, '__updated_ms': 1771172745172, '__version': 4, '__deleted': 1}
  {'id': 3, '__added_ms': 1771172745180, '__updated_ms': 1771172745180, '__version': 5, '__deleted': 0}

Triggers before restore:
  ['_history_json_items_delete', '_history_json_items_insert', '_history_json_items_update', 'chronicle_items_ad', 'chronicle_items_ai', 'chronicle_items_au', 'chronicle_items_bi']

=== Restoring to up_to_id=3 (before delete) WITH swap=True ===
Restore returned: items

Items table after swap:
  {'id': 1, 'name': 'Widget', 'price': 14.99}
  {'id': 2, 'name': 'Gadget', 'price': 19.99}

Triggers after swap:
  []

Is chronicle still enabled? True

Chronicle table after swap:
  {'id': 1, '__added_ms': 1771172745157, '__updated_ms': 1771172745165, '__version': 3, '__deleted': 0}
  {'id': 2, '__added_ms': 1771172745158, '__updated_ms': 1771172745172, '__version': 4, '__deleted': 1}
  {'id': 3, '__added_ms': 1771172745180, '__updated_ms': 1771172745180, '__version': 5, '__deleted': 0}

=== Testing operations after swap ===
Insert after swap succeeded
Chronicle has 3 rows
  {'id': 1, '__added_ms': 1771172745157, '__updated_ms': 1771172745165, '__version': 3, '__deleted': 0}
  {'id': 2, '__added_ms': 1771172745158, '__updated_ms': 1771172745172, '__version': 4, '__deleted': 1}
  {'id': 3, '__added_ms': 1771172745180, '__updated_ms': 1771172745180, '__version': 5, '__deleted': 0}
History-json latest: {'id': 5, 'timestamp': '2026-02-15 16:25:45.180', 'operation': 'insert', 'pk_id': 3, 'updated_values': '{"name":"Doohickey","price":29.99}', 'group': None}

Restore swap test completed!
```

### Detailed swap trigger analysis

Let's prove exactly what happens to triggers after swap, and show the fix.

```bash
/usr/local/bin/python3 test_restore_swap_detail.py
```

```output
Triggers BEFORE swap (7):
  _history_json_items_delete
  _history_json_items_insert
  _history_json_items_update
  chronicle_items_ad
  chronicle_items_ai
  chronicle_items_au
  chronicle_items_bi

Chronicle max version before: 3
History-json entry count before: 3

=== Performing restore(swap=True) ===

Triggers AFTER swap (0):

*** ALL TRIGGERS DESTROYED BY restore(swap=True) ***

=== Attempting operations after swap (no triggers active) ===
Chronicle max version after insert: 3 (was 3)
History-json entry count after insert: 3 (was 3)
*** Chronicle DID NOT track the insert (triggers gone!) ***
*** History-json DID NOT track the insert (triggers gone!) ***

is_chronicle_enabled() says: True
(This is MISLEADING - it checks for the table, not triggers)

=== Fix: Re-enable both after swap ===
Triggers after re-enable (3):
  _history_json_items_delete
  _history_json_items_insert
  _history_json_items_update

Test insert after re-enable:
Chronicle max version: 3
History-json entry count: 4
History-json is tracking again!

Detailed swap test completed!
```

### Fixing triggers after swap

Since chronicle's enable_chronicle() is idempotent when the table exists, you must disable_chronicle() first, then re-enable. Let's verify:

```bash
/usr/local/bin/python3 test_swap_fix.py
```

```output
After swap: 0 triggers

=== Approach 1: disable then re-enable chronicle ===
is_chronicle_enabled: True
After disable: is_chronicle_enabled = False
After re-enable: is_chronicle_enabled = True

Triggers after proper fix (7):
  _history_json_items_delete
  _history_json_items_insert
  _history_json_items_update
  chronicle_items_ad
  chronicle_items_ai
  chronicle_items_au
  chronicle_items_bi

Test operations:
Chronicle max version: 2
History-json entries: 3

Chronicle rows:
  {'id': 1, '__added_ms': 1771172807406, '__updated_ms': 1771172807406, '__version': 1, '__deleted': 0}
  {'id': 3, '__added_ms': 1771172807422, '__updated_ms': 1771172807422, '__version': 2, '__deleted': 0}

History-json latest:
  {'id': 3, 'timestamp': '2026-02-15 16:26:47.422', 'operation': 'insert', 'pk_id': 3, 'updated_values': '{"name":"Test","price":99.99}', 'group': None}

Proper fix after swap verified!
```

## Part 10: Performance Comparison

How much overhead do the triggers add? Testing 1000 inserts, 1000 updates, and 1000 deletes with: no tracking, chronicle only, history-json only, and both together.

```bash
/usr/local/bin/python3 test_bulk_performance.py
```

```output
Benchmark: 1000 inserts, 1000 updates, 1000 deletes

  No tracking: 24.1ms
  Chronicle only: 36.8ms
  History-json only: 41.2ms
  Both libraries: 55.3ms

Overhead vs baseline:
  Chronicle only: 1.53x
  History-json only: 1.71x
  Both libraries: 2.30x

Data integrity (both libraries, 1000 ops each):
  Items remaining: 0 (expected: 0 after delete all)
  Chronicle rows: 1000 (expected: 1000 - one per PK)
  History-json entries: 3000 (expected: 3000 - one per operation)
  Chronicle deleted entries: 1000 (expected: 1000)

Benchmark completed!
```

## Part 11: Multiple Tables with Shared Change Groups

Testing both libraries across three tables, including history-json's change_group() spanning multiple tables.

```bash
/usr/local/bin/python3 test_multiple_tables.py
```

```output
=== Enable both libraries on all three tables ===
Total triggers: 21
Total tables: 11
Tables: ['_chronicle_orders', '_chronicle_products', '_chronicle_users', '_chroniclesnapshots', '_history_json', '_history_json_orders', '_history_json_products', '_history_json_users', 'orders', 'products', 'users']

Chronicled tables: ['users', 'orders', 'products']

=== Cross-table operations ===
All three tables tracked successfully.

Chronicle - users:
  {'id': 1, '__added_ms': 1771172875602, '__updated_ms': 1771172875611, '__version': 2, '__deleted': 0}
Chronicle - orders:
  {'id': 1000, '__added_ms': 1771172875603, '__updated_ms': 1771172875611, '__version': 2, '__deleted': 0}
Chronicle - products:
  {'id': 100, '__added_ms': 1771172875603, '__updated_ms': 1771172875603, '__version': 1, '__deleted': 0}

History-json - users:
  {'id': 1, 'timestamp': '2026-02-15 16:27:55.602', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Alice","email":"alice@example.com"}', 'group': None}
  {'id': 2, 'timestamp': '2026-02-15 16:27:55.611', 'operation': 'update', 'pk_id': 1, 'updated_values': '{"email":"alice@new.com"}', 'group': None}
History-json - orders:
  {'id': 1, 'timestamp': '2026-02-15 16:27:55.603', 'operation': 'insert', 'pk_id': 1000, 'updated_values': '{"user_id":1,"total":9.99}', 'group': None}
  {'id': 2, 'timestamp': '2026-02-15 16:27:55.611', 'operation': 'update', 'pk_id': 1000, 'updated_values': '{"total":19.98}', 'group': None}

=== Change group spanning multiple tables ===
Change group 1 applied across all tables.

History-json users (latest):
  {'id': 3, 'timestamp': '2026-02-15 16:27:55.621', 'operation': 'update', 'pk_id': 1, 'updated_values': '{"name":"Alice Smith"}', 'group': 1}
History-json orders (latest):
  {'id': 3, 'timestamp': '2026-02-15 16:27:55.621', 'operation': 'insert', 'pk_id': 1001, 'updated_values': '{"user_id":1,"total":19.99}', 'group': 1}
History-json products (latest):
  {'id': 2, 'timestamp': '2026-02-15 16:27:55.620', 'operation': 'insert', 'pk_id': 101, 'updated_values': '{"name":"Gadget","price":19.99}', 'group': 1}

(All three share the same group id from _history_json table)
Groups table: 1 groups
  {'id': 1, 'note': 'Cross-table update', 'current': None}

Multiple table tests completed successfully!
```

## Part 12: INSERT OR REPLACE Deep Dive

INSERT OR REPLACE in SQLite actually performs DELETE + INSERT when there's a PK conflict. How does each library handle this, and do they interfere with each other's handling?

```bash
/usr/local/bin/python3 test_insert_or_replace_detail.py
```

```output
=== Insert initial row ===

Chronicle:
  {'id': 1, '__added_ms': 1771172910978, '__updated_ms': 1771172910978, '__version': 1, '__deleted': 0}
History-json:
  {'id': 1, 'timestamp': '2026-02-15 16:28:30.979', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget","price":9.99}', 'group': None}

=== INSERT OR REPLACE with changed data ===

Chronicle (should update version, not create duplicate):
  {'id': 1, '__added_ms': 1771172910978, '__updated_ms': 1771172910987, '__version': 2, '__deleted': 0}

History-json (what does it record for a REPLACE?):
  {'id': 1, 'timestamp': '2026-02-15 16:28:30.979', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget","price":9.99}', 'group': None}
  {'id': 2, 'timestamp': '2026-02-15 16:28:30.987', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget Pro","price":14.99}', 'group': None}

=== Key observation ===
History-json recorded operations for pk=1: ['insert', 'insert']
(INSERT OR REPLACE triggers DELETE then INSERT in SQLite)
Delete entries: 0, Insert entries: 2

=== INSERT OR REPLACE with identical data ===
Chronicle version: 2 -> 2
History-json entries: 2 -> 3
Chronicle: SMART - detected no real change, no version bump
History-json: Recorded 1 new entries for identical data:
  {'id': 3, 'timestamp': '2026-02-15 16:28:30.995', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget Pro","price":14.99}', 'group': None}

=== Verify table ===
  (1, 'Widget Pro', 14.99)

INSERT OR REPLACE analysis completed!
```

## Part 13: Recursive Triggers and INSERT OR REPLACE

A subtle but important detail: SQLite only fires DELETE triggers during INSERT OR REPLACE when recursive_triggers is ON. By default it's OFF. This affects how each library records these operations.

```bash
/usr/local/bin/python3 test_recursive_triggers.py
```

```output

============================================================
recursive_triggers = 0 (OFF (default))
============================================================

After initial insert:
  History entries: 1

Performing INSERT OR REPLACE with changed data...
  Success!

History-json entries:
  {'id': 1, 'timestamp': '2026-02-15 16:29:41.449', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget","price":9.99}', 'group': None}
  {'id': 2, 'timestamp': '2026-02-15 16:29:41.456', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget Pro","price":14.99}', 'group': None}

Operations recorded: ['insert', 'insert']
  Deletes: 0, Inserts: 2
  -> DELETE trigger did NOT fire during INSERT OR REPLACE

Chronicle state:
  {'id': 1, '__added_ms': 1771172981448, '__updated_ms': 1771172981455, '__version': 2, '__deleted': 0}

Items table:
  (1, 'Widget Pro', 14.99)

============================================================
recursive_triggers = 1 (ON)
============================================================

After initial insert:
  History entries: 1

Performing INSERT OR REPLACE with changed data...
  Success!

History-json entries:
  {'id': 1, 'timestamp': '2026-02-15 16:29:41.499', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget","price":9.99}', 'group': None}
  {'id': 2, 'timestamp': '2026-02-15 16:29:41.507', 'operation': 'delete', 'pk_id': 1, 'updated_values': None, 'group': None}
  {'id': 3, 'timestamp': '2026-02-15 16:29:41.507', 'operation': 'insert', 'pk_id': 1, 'updated_values': '{"name":"Widget Pro","price":14.99}', 'group': None}

Operations recorded: ['insert', 'delete', 'insert']
  Deletes: 1, Inserts: 2
  -> DELETE trigger FIRED during INSERT OR REPLACE

Chronicle state:
  {'id': 1, '__added_ms': 1771172981498, '__updated_ms': 1771172981507, '__version': 2, '__deleted': 0}

Items table:
  (1, 'Widget Pro', 14.99)


=== Summary ===
With recursive_triggers OFF (default):
  INSERT OR REPLACE -> history-json sees: insert, insert
  DELETE trigger does NOT fire for the implicit delete
  Chronicle handles this correctly via BEFORE INSERT snapshot

With recursive_triggers ON:
  INSERT OR REPLACE -> history-json sees: insert, delete, insert
  DELETE trigger FIRES for the implicit delete
  Both libraries must handle the additional trigger execution
```

## Conclusions

Both libraries coexist cleanly on the same table for standard operations. They create separate triggers (7 per table) with no naming conflicts. Enable order doesn't matter. Performance overhead is roughly additive (~2.3x baseline with both).

**Key gotcha:** `restore(swap=True)` destroys ALL triggers (both libraries). After a swap, you must `disable_chronicle()` + `enable_chronicle()` + `enable_tracking()` to restore tracking.

**Subtle difference:** INSERT OR REPLACE behavior depends on `recursive_triggers` pragma. With it OFF (default), history-json doesn't see the implicit delete. Chronicle handles both settings correctly via its BEFORE INSERT snapshot mechanism.

**No-op detection:** Chronicle skips version bumps for no-op updates/replaces. History-json records every operation, even if nothing changed.
