"""Core implementation of sqlite-history-json.

Implements the "updated values" JSON audit log pattern for SQLite tables,
using triggers to record INSERT, UPDATE, and DELETE operations.
"""

from __future__ import annotations

import json
import sqlite3


def _audit_table_name(table_name: str) -> str:
    return f"_history_json_{table_name}"


def _audit_pk_col_name(source_col_name: str) -> str:
    """Return the audit table column name for a source PK column."""
    return f"pk_{source_col_name}"


def _get_table_info(conn: sqlite3.Connection, table_name: str) -> list[dict]:
    """Return column info for a table via PRAGMA table_info."""
    rows = conn.execute(f"PRAGMA table_info([{table_name}])").fetchall()
    return [
        {"cid": r[0], "name": r[1], "type": r[2], "notnull": r[3], "pk": r[5]}
        for r in rows
    ]


def _get_pk_columns(columns: list[dict]) -> list[dict]:
    """Return the primary key columns, ordered by pk index."""
    pks = [c for c in columns if c["pk"] > 0]
    pks.sort(key=lambda c: c["pk"])
    return pks


def _get_non_pk_columns(columns: list[dict]) -> list[dict]:
    """Return the non-primary-key columns."""
    return [c for c in columns if c["pk"] == 0]


def _is_blob_type(col_type: str) -> bool:
    """Check if a column type is BLOB."""
    return col_type.upper() == "BLOB"


def _build_insert_trigger_sql(
    table_name: str,
    audit_name: str,
    pk_cols: list[dict],
    non_pk_cols: list[dict],
) -> str:
    """Build the AFTER INSERT trigger SQL."""
    audit_pk_col_names = ", ".join(
        f"[{_audit_pk_col_name(c['name'])}]" for c in pk_cols
    )
    pk_new_refs = ", ".join(f"NEW.[{c['name']}]" for c in pk_cols)

    # Build json_object arguments for all non-PK columns
    json_args = []
    for col in non_pk_cols:
        name = col["name"]
        if _is_blob_type(col["type"]):
            val_expr = (
                f"CASE WHEN NEW.[{name}] IS NULL "
                f"THEN json_object('null', 1) "
                f"ELSE json_object('hex', hex(NEW.[{name}])) END"
            )
        else:
            val_expr = (
                f"CASE WHEN NEW.[{name}] IS NULL "
                f"THEN json_object('null', 1) "
                f"ELSE NEW.[{name}] END"
            )
        json_args.append(f"'{name}', {val_expr}")

    json_obj = f"json_object({', '.join(json_args)})" if json_args else "'{{}}'"

    return f"""CREATE TRIGGER IF NOT EXISTS [{audit_name}_insert]
AFTER INSERT ON [{table_name}]
BEGIN
    INSERT INTO [{audit_name}] (timestamp, operation, {audit_pk_col_names}, updated_values)
    VALUES (
        strftime('%Y-%m-%d %H:%M:%f', 'now'),
        'insert',
        {pk_new_refs},
        {json_obj}
    );
END;"""


def _build_update_trigger_sql(
    table_name: str,
    audit_name: str,
    pk_cols: list[dict],
    non_pk_cols: list[dict],
) -> str:
    """Build the AFTER UPDATE trigger SQL using nested json_patch."""
    audit_pk_col_names = ", ".join(
        f"[{_audit_pk_col_name(c['name'])}]" for c in pk_cols
    )
    pk_new_refs = ", ".join(f"NEW.[{c['name']}]" for c in pk_cols)

    if not non_pk_cols:
        json_expr = "'{}'"
    else:
        def case_for_col(col: dict) -> str:
            name = col["name"]
            if _is_blob_type(col["type"]):
                return (
                    f"CASE\n"
                    f"                WHEN OLD.[{name}] IS NOT NEW.[{name}] THEN\n"
                    f"                    CASE\n"
                    f"                        WHEN NEW.[{name}] IS NULL THEN json_object('{name}', json_object('null', 1))\n"
                    f"                        ELSE json_object('{name}', json_object('hex', hex(NEW.[{name}])))\n"
                    f"                    END\n"
                    f"                ELSE '{{}}'\n"
                    f"            END"
                )
            else:
                return (
                    f"CASE\n"
                    f"                WHEN OLD.[{name}] IS NOT NEW.[{name}] THEN\n"
                    f"                    CASE\n"
                    f"                        WHEN NEW.[{name}] IS NULL THEN json_object('{name}', json_object('null', 1))\n"
                    f"                        ELSE json_object('{name}', NEW.[{name}])\n"
                    f"                    END\n"
                    f"                ELSE '{{}}'\n"
                    f"            END"
                )

        expr = "'{}'"
        for col in non_pk_cols:
            case = case_for_col(col)
            expr = f"json_patch(\n            {expr},\n            {case}\n        )"

        json_expr = expr

    return f"""CREATE TRIGGER IF NOT EXISTS [{audit_name}_update]
AFTER UPDATE ON [{table_name}]
BEGIN
    INSERT INTO [{audit_name}] (timestamp, operation, {audit_pk_col_names}, updated_values)
    VALUES (
        strftime('%Y-%m-%d %H:%M:%f', 'now'),
        'update',
        {pk_new_refs},
        {json_expr}
    );
END;"""


def _build_delete_trigger_sql(
    table_name: str,
    audit_name: str,
    pk_cols: list[dict],
) -> str:
    """Build the AFTER DELETE trigger SQL."""
    audit_pk_col_names = ", ".join(
        f"[{_audit_pk_col_name(c['name'])}]" for c in pk_cols
    )
    pk_old_refs = ", ".join(f"OLD.[{c['name']}]" for c in pk_cols)

    return f"""CREATE TRIGGER IF NOT EXISTS [{audit_name}_delete]
AFTER DELETE ON [{table_name}]
BEGIN
    INSERT INTO [{audit_name}] (timestamp, operation, {audit_pk_col_names}, updated_values)
    VALUES (
        strftime('%Y-%m-%d %H:%M:%f', 'now'),
        'delete',
        {pk_old_refs},
        NULL
    );
END;"""


def enable_tracking(
    conn: sqlite3.Connection,
    table_name: str,
    *,
    populate_table: bool = True,
) -> None:
    """Create audit table and triggers for the given table.

    The audit table is named ``_history_json_{table_name}`` and contains:
    - id: auto-incrementing primary key
    - timestamp: ISO-8601 datetime with microsecond precision
    - operation: 'insert', 'update', or 'delete'
    - pk_{col} columns for each primary key column of the source table
    - updated_values: JSON text of changed column values

    Args:
        conn: SQLite connection.
        table_name: Name of the table to track.
        populate_table: If True (the default), snapshot all existing rows into
            the audit log so history is complete from this point.

    This is idempotent: calling it twice has no additional effect.
    """
    columns = _get_table_info(conn, table_name)
    pk_cols = _get_pk_columns(columns)
    non_pk_cols = _get_non_pk_columns(columns)
    audit_name = _audit_table_name(table_name)

    if not pk_cols:
        raise ValueError(
            f"Table {table_name!r} has no explicit primary key. "
            "sqlite-history-json requires an explicit PRIMARY KEY."
        )

    # Build audit table PK column definitions with pk_ prefix
    pk_col_defs = ", ".join(
        f"[{_audit_pk_col_name(c['name'])}] {c['type']}" for c in pk_cols
    )

    create_audit = f"""CREATE TABLE IF NOT EXISTS [{audit_name}] (
    id INTEGER PRIMARY KEY,
    timestamp TEXT,
    operation TEXT,
    {pk_col_defs},
    updated_values TEXT
);"""

    conn.execute(create_audit)

    # Build and create triggers
    insert_sql = _build_insert_trigger_sql(
        table_name, audit_name, pk_cols, non_pk_cols
    )
    update_sql = _build_update_trigger_sql(
        table_name, audit_name, pk_cols, non_pk_cols
    )
    delete_sql = _build_delete_trigger_sql(table_name, audit_name, pk_cols)

    conn.execute(insert_sql)
    conn.execute(update_sql)
    conn.execute(delete_sql)

    # Create indexes for common query patterns
    conn.execute(
        f"CREATE INDEX IF NOT EXISTS [{audit_name}_timestamp] "
        f"ON [{audit_name}] (timestamp)"
    )
    audit_pk_col_names_str = ", ".join(
        f"[{_audit_pk_col_name(c['name'])}]" for c in pk_cols
    )
    conn.execute(
        f"CREATE INDEX IF NOT EXISTS [{audit_name}_pk] "
        f"ON [{audit_name}] ({audit_pk_col_names_str})"
    )

    if populate_table:
        # Only populate if audit table is empty (preserves idempotency)
        count = conn.execute(
            f"SELECT count(*) FROM [{audit_name}]"
        ).fetchone()[0]
        if count == 0:
            populate(conn, table_name)


def disable_tracking(conn: sqlite3.Connection, table_name: str) -> None:
    """Drop triggers for the given table. Keeps the audit table intact.

    This is idempotent: calling it when no triggers exist has no effect.
    """
    audit_name = _audit_table_name(table_name)
    conn.execute(f"DROP TRIGGER IF EXISTS [{audit_name}_insert]")
    conn.execute(f"DROP TRIGGER IF EXISTS [{audit_name}_update]")
    conn.execute(f"DROP TRIGGER IF EXISTS [{audit_name}_delete]")


def populate(conn: sqlite3.Connection, table_name: str) -> None:
    """Populate the audit log with a snapshot of the current table state.

    For each existing row, creates an 'insert' entry in the audit log
    containing all column values. This makes the audit log self-contained
    for reconstruction purposes.

    The audit table and triggers must already exist (call enable_tracking first).
    """
    columns = _get_table_info(conn, table_name)
    pk_cols = _get_pk_columns(columns)
    non_pk_cols = _get_non_pk_columns(columns)
    audit_name = _audit_table_name(table_name)

    # Read all current rows
    all_col_names = ", ".join(f"[{c['name']}]" for c in columns)
    rows = conn.execute(f"SELECT {all_col_names} FROM [{table_name}]").fetchall()

    for row in rows:
        row_dict = {}
        for i, col in enumerate(columns):
            row_dict[col["name"]] = row[i]

        # Build the PK values with pk_ prefix column names
        pk_insert_cols = ", ".join(
            f"[{_audit_pk_col_name(c['name'])}]" for c in pk_cols
        )
        pk_insert_params = ", ".join("?" for _ in pk_cols)
        pk_values = [row_dict[c["name"]] for c in pk_cols]

        # Build JSON for non-PK columns
        json_dict = {}
        for col in non_pk_cols:
            val = row_dict[col["name"]]
            if val is None:
                json_dict[col["name"]] = {"null": 1}
            elif isinstance(val, bytes):
                json_dict[col["name"]] = {"hex": val.hex().upper()}
            else:
                json_dict[col["name"]] = val

        json_str = json.dumps(json_dict)

        conn.execute(
            f"INSERT INTO [{audit_name}] (timestamp, operation, {pk_insert_cols}, updated_values) "
            f"VALUES (strftime('%Y-%m-%d %H:%M:%f', 'now'), 'insert', {pk_insert_params}, ?)",
            pk_values + [json_str],
        )


def _decode_json_value(val):
    """Decode a JSON value from the audit log, handling null and hex conventions."""
    if isinstance(val, dict):
        if "null" in val:
            return None
        if "hex" in val:
            return bytes.fromhex(val["hex"])
    return val


def restore(
    conn: sqlite3.Connection,
    table_name: str,
    *,
    timestamp: str | None = None,
    up_to_id: int | None = None,
    new_table_name: str | None = None,
    swap: bool = False,
) -> str:
    """Restore a table to its state at the given timestamp or audit entry ID.

    Replays audit log entries to reconstruct the table state. Filter by
    either ``timestamp`` (inclusive) or ``up_to_id`` (inclusive). If both
    are provided, both conditions must be satisfied.

    Args:
        conn: SQLite connection.
        table_name: Name of the tracked table.
        timestamp: ISO-8601 timestamp to restore to (inclusive).
        up_to_id: Audit log entry ID to restore up to (inclusive).
        new_table_name: Name for the restored table. If None, auto-generated.
        swap: If True, atomically swap the restored table with the original.

    Returns:
        The name of the restored table (equals table_name if swap=True).
    """
    columns = _get_table_info(conn, table_name)
    pk_cols = _get_pk_columns(columns)
    non_pk_cols = _get_non_pk_columns(columns)
    audit_name = _audit_table_name(table_name)

    if new_table_name is None and not swap:
        new_table_name = f"{table_name}_restored"

    target_name = new_table_name if not swap else f"_tmp_restore_{table_name}"

    # Create target table with same schema
    create_sql = conn.execute(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name=?",
        (table_name,),
    ).fetchone()[0]

    # Replace the table name in the CREATE statement
    target_create = create_sql.replace(
        f"CREATE TABLE {table_name}", f"CREATE TABLE [{target_name}]", 1
    )
    if target_create == create_sql:
        target_create = create_sql.replace(
            f'CREATE TABLE "{table_name}"', f"CREATE TABLE [{target_name}]", 1
        )
    if target_create == create_sql:
        target_create = create_sql.replace(
            f"CREATE TABLE [{table_name}]", f"CREATE TABLE [{target_name}]", 1
        )

    # Drop if already exists
    conn.execute(f"DROP TABLE IF EXISTS [{target_name}]")
    conn.execute(target_create)

    # Read audit log entries up to the specified point
    conditions = []
    params: list = []
    if timestamp is not None:
        conditions.append("timestamp <= ?")
        params.append(timestamp)
    if up_to_id is not None:
        conditions.append("id <= ?")
        params.append(up_to_id)
    where_clause = f" WHERE {' AND '.join(conditions)}" if conditions else ""
    audit_rows = conn.execute(
        f"SELECT * FROM [{audit_name}]{where_clause} ORDER BY id",
        params,
    ).fetchall()

    # Get audit column names
    audit_col_names = [desc[0] for desc in conn.execute(
        f"SELECT * FROM [{audit_name}] LIMIT 0"
    ).description]

    for audit_row in audit_rows:
        row_dict = dict(zip(audit_col_names, audit_row))
        operation = row_dict["operation"]

        # Get PK values from audit row (pk_ prefixed columns)
        pk_where = " AND ".join(f"[{c['name']}] = ?" for c in pk_cols)
        pk_values = [row_dict[_audit_pk_col_name(c["name"])] for c in pk_cols]

        if operation == "insert":
            updated_values = json.loads(row_dict["updated_values"])
            # Build full row: PK values + decoded non-PK values
            all_cols = []
            all_vals = []
            for col in pk_cols:
                all_cols.append(f"[{col['name']}]")
                all_vals.append(row_dict[_audit_pk_col_name(col["name"])])
            for col in non_pk_cols:
                all_cols.append(f"[{col['name']}]")
                if col["name"] in updated_values:
                    all_vals.append(_decode_json_value(updated_values[col["name"]]))
                else:
                    all_vals.append(None)

            placeholders = ", ".join("?" for _ in all_cols)
            conn.execute(
                f"INSERT INTO [{target_name}] ({', '.join(all_cols)}) VALUES ({placeholders})",
                all_vals,
            )

        elif operation == "update":
            updated_values = json.loads(row_dict["updated_values"])
            if not updated_values:
                continue  # No actual changes
            set_clauses = []
            set_vals = []
            for col_name, val in updated_values.items():
                set_clauses.append(f"[{col_name}] = ?")
                set_vals.append(_decode_json_value(val))

            conn.execute(
                f"UPDATE [{target_name}] SET {', '.join(set_clauses)} WHERE {pk_where}",
                set_vals + pk_values,
            )

        elif operation == "delete":
            conn.execute(
                f"DELETE FROM [{target_name}] WHERE {pk_where}",
                pk_values,
            )

    if swap:
        old_backup = f"_tmp_old_{table_name}"
        conn.execute(f"DROP TABLE IF EXISTS [{old_backup}]")
        conn.execute(f"ALTER TABLE [{table_name}] RENAME TO [{old_backup}]")
        conn.execute(f"ALTER TABLE [{target_name}] RENAME TO [{table_name}]")
        conn.execute(f"DROP TABLE [{old_backup}]")
        return table_name

    return target_name
