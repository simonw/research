#!/usr/bin/env python3
"""
Demo script showing sqlite_linter in action
"""

import sqlite_linter
from sqlite_linter import LintError, LintWarning


def demo_basic_usage():
    """Basic usage example"""
    print("=" * 60)
    print("DEMO 1: Basic Usage")
    print("=" * 60)

    # Create a linting connection
    conn = sqlite_linter.connect(":memory:")

    # Create a table
    conn.execute("""
        CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name TEXT,
            price REAL
        )
    """)

    # Insert some data
    conn.execute("INSERT INTO products (name, price) VALUES ('Widget', 9.99)")
    conn.execute("INSERT INTO products (name, price) VALUES ('Gadget', 19.99)")

    # Valid query works fine
    print("\n✓ Valid query:")
    print("  SELECT name, price FROM products")
    cursor = conn.execute("SELECT name, price FROM products")
    for row in cursor:
        print(f"    {row}")

    # Invalid CAST is caught
    print("\n✗ Invalid query (STRING instead of TEXT):")
    print("  SELECT CAST(price AS STRING) FROM products")
    try:
        conn.execute("SELECT CAST(price AS STRING) FROM products")
    except LintError as e:
        print(f"    LintError: {e}")

    conn.close()


def demo_warning_levels():
    """Demo showing warning vs error levels"""
    print("\n" + "=" * 60)
    print("DEMO 2: Warning Levels")
    print("=" * 60)

    # By default, warnings don't block execution
    conn = sqlite_linter.connect(":memory:", raise_on_warning=False)
    conn.execute("CREATE TABLE users (id INTEGER, name TEXT)")

    print("\n⚠ Warning (but allowed):")
    print("  SELECT * FROM users")
    cursor = conn.cursor()
    cursor.execute("SELECT * FROM users")
    print(f"    Query executed, {len(cursor.last_issues)} warnings found:")
    for issue in cursor.last_issues:
        print(f"      - {issue.message}")

    # Can configure to block on warnings
    print("\n⚠ Same query with raise_on_warning=True:")
    conn2 = sqlite_linter.connect(":memory:", raise_on_warning=True)
    conn2.execute("CREATE TABLE users (id INTEGER, name TEXT)")
    try:
        conn2.execute("SELECT * FROM users")
    except LintWarning as e:
        print(f"    LintWarning: {e}")

    conn.close()
    conn2.close()


def demo_custom_rules():
    """Demo showing custom rule configuration"""
    print("\n" + "=" * 60)
    print("DEMO 3: Custom Rules")
    print("=" * 60)

    # Only check for CAST errors
    from sqlite_linter import InvalidCastTypeRule
    rules = [InvalidCastTypeRule()]

    conn = sqlite_linter.connect(":memory:", rules=rules)
    conn.execute("CREATE TABLE data (value INTEGER)")

    print("\n✓ SELECT * is allowed (SelectStarRule not included):")
    print("  SELECT * FROM data")
    cursor = conn.execute("SELECT * FROM data")
    print("    Query executed successfully")

    print("\n✗ Invalid CAST still caught:")
    print("  SELECT CAST(value AS STRING) FROM data")
    try:
        conn.execute("SELECT CAST(value AS STRING) FROM data")
    except LintError as e:
        print(f"    LintError: {e}")

    conn.close()


def demo_dangerous_operations():
    """Demo showing detection of dangerous operations"""
    print("\n" + "=" * 60)
    print("DEMO 4: Dangerous Operations")
    print("=" * 60)

    conn = sqlite_linter.connect(":memory:", raise_on_warning=False)
    conn.execute("CREATE TABLE accounts (id INTEGER, balance REAL)")
    conn.execute("INSERT INTO accounts VALUES (1, 100.0)")
    conn.execute("INSERT INTO accounts VALUES (2, 200.0)")

    print("\n⚠ DELETE without WHERE:")
    print("  DELETE FROM accounts")
    cursor = conn.cursor()
    cursor.execute("DELETE FROM accounts")
    print(f"    Warning: {cursor.last_issues[0].message}")

    conn.close()


def demo_invalid_functions():
    """Demo showing invalid function detection"""
    print("\n" + "=" * 60)
    print("DEMO 5: Invalid Functions")
    print("=" * 60)

    conn = sqlite_linter.connect(":memory:")
    conn.execute("CREATE TABLE users (first_name TEXT, last_name TEXT)")

    print("\n✗ Invalid function CONCAT:")
    print("  SELECT CONCAT(first_name, last_name) FROM users")
    try:
        conn.execute("SELECT CONCAT(first_name, last_name) FROM users")
    except LintError as e:
        print(f"    LintError: {e}")

    print("\n✓ Correct SQLite syntax:")
    print("  SELECT first_name || ' ' || last_name FROM users")
    cursor = conn.execute("SELECT first_name || ' ' || last_name FROM users")
    print("    Query executed successfully")

    conn.close()


def demo_permissive_mode():
    """Demo showing permissive mode for checking without blocking"""
    print("\n" + "=" * 60)
    print("DEMO 6: Permissive Mode (Audit Only)")
    print("=" * 60)

    # Don't block on any issues, just track them
    conn = sqlite_linter.connect(":memory:",
                                 raise_on_error=False,
                                 raise_on_warning=False)
    conn.execute("CREATE TABLE test (value TEXT)")

    print("\nRunning problematic queries without blocking:\n")

    queries = [
        "SELECT * FROM test",
        "SELECT CAST(value AS STRING) FROM test",
        "DELETE FROM test",
    ]

    for query in queries:
        cursor = conn.cursor()
        try:
            cursor.execute(query)
            if cursor.last_issues:
                print(f"Query: {query}")
                for issue in cursor.last_issues:
                    icon = "✗" if issue.level.value == "error" else "⚠"
                    print(f"  {icon} [{issue.level.value.upper()}] {issue.message}")
            else:
                print(f"Query: {query}")
                print("  ✓ No issues")
        except Exception as e:
            print(f"Query: {query}")
            print(f"  ✗ SQLite error: {e}")
        print()

    conn.close()


if __name__ == "__main__":
    demo_basic_usage()
    demo_warning_levels()
    demo_custom_rules()
    demo_dangerous_operations()
    demo_invalid_functions()
    demo_permissive_mode()

    print("\n" + "=" * 60)
    print("All demos completed!")
    print("=" * 60)
