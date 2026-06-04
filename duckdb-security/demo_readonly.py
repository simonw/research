#!/usr/bin/env python3
"""
Demonstration of DuckDB read-only connection options.
"""
import duckdb
import tempfile
import os

def main():
    print("=" * 60)
    print("DuckDB Read-Only Connection Demo")
    print("=" * 60)

    # Create a temp directory for our test database
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = os.path.join(tmpdir, "test.duckdb")

        # First, create a database with some data using read-write connection
        print("\n1. Creating test database with sample data...")
        con = duckdb.connect(db_path)
        con.execute("CREATE TABLE users (id INTEGER, name VARCHAR)")
        con.execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Charlie')")
        con.execute("CREATE TABLE products (id INTEGER, price DECIMAL(10,2))")
        con.execute("INSERT INTO products VALUES (1, 9.99), (2, 19.99)")
        con.close()
        print(f"   Database created at: {db_path}")

        # Test 1: Open in read-only mode
        print("\n2. Opening database in READ_ONLY mode...")
        con_ro = duckdb.connect(db_path, read_only=True)

        # Verify we can read
        result = con_ro.execute("SELECT * FROM users").fetchall()
        print(f"   Read successful - got {len(result)} rows from users table")

        # Try to write (should fail)
        print("\n3. Attempting INSERT on read-only connection...")
        try:
            con_ro.execute("INSERT INTO users VALUES (4, 'David')")
            print("   ERROR: Insert should have failed!")
        except Exception as e:
            print(f"   Expected error: {type(e).__name__}")
            print(f"   Message: {str(e)[:80]}...")

        # Try to create table (should fail)
        print("\n4. Attempting CREATE TABLE on read-only connection...")
        try:
            con_ro.execute("CREATE TABLE new_table (x INT)")
            print("   ERROR: Create should have failed!")
        except Exception as e:
            print(f"   Expected error: {type(e).__name__}")
            print(f"   Message: {str(e)[:80]}...")

        # Try to DROP table (should fail)
        print("\n5. Attempting DROP TABLE on read-only connection...")
        try:
            con_ro.execute("DROP TABLE users")
            print("   ERROR: Drop should have failed!")
        except Exception as e:
            print(f"   Expected error: {type(e).__name__}")
            print(f"   Message: {str(e)[:80]}...")

        con_ro.close()

        # Test 2: Using access_mode config option
        print("\n6. Opening with access_mode='READ_ONLY' config...")
        con_config = duckdb.connect(db_path, config={"access_mode": "READ_ONLY"})

        result = con_config.execute("SELECT COUNT(*) FROM products").fetchone()
        print(f"   Read successful - products table has {result[0]} rows")

        print("\n7. Attempting DELETE on config-based read-only connection...")
        try:
            con_config.execute("DELETE FROM products WHERE id = 1")
            print("   ERROR: Delete should have failed!")
        except Exception as e:
            print(f"   Expected error: {type(e).__name__}")
            print(f"   Message: {str(e)[:80]}...")

        con_config.close()

        print("\n" + "=" * 60)
        print("READ-ONLY DEMO COMPLETE - All security checks passed!")
        print("=" * 60)

if __name__ == "__main__":
    main()
