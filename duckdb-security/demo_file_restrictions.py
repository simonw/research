#!/usr/bin/env python3
"""
Demonstration of DuckDB file/network access restrictions.
"""
import duckdb
import tempfile
import os

def demo_disable_external_access():
    """Demonstrate disabling all external access."""
    print("\n" + "=" * 60)
    print("Demo 1: Disable External Access (enable_external_access=false)")
    print("=" * 60)

    con = duckdb.connect(":memory:", config={"enable_external_access": "false"})

    # Create in-memory table
    con.execute("CREATE TABLE test (x INT)")
    con.execute("INSERT INTO test VALUES (1), (2), (3)")
    print("\n   In-memory tables work fine...")

    # Try to read a local file
    print("\n   Attempting to read local CSV file...")
    try:
        con.execute("SELECT * FROM read_csv('/etc/passwd')")
        print("   ERROR: Should have been blocked!")
    except Exception as e:
        print(f"   Blocked! {type(e).__name__}: {str(e)[:70]}...")

    # Try to read from HTTPS
    print("\n   Attempting to read from HTTPS URL...")
    try:
        con.execute("SELECT * FROM read_csv('https://example.com/data.csv')")
        print("   ERROR: Should have been blocked!")
    except Exception as e:
        print(f"   Blocked! {type(e).__name__}: {str(e)[:70]}...")

    # Try COPY TO
    print("\n   Attempting COPY TO file...")
    try:
        con.execute("COPY test TO '/tmp/output.csv'")
        print("   ERROR: Should have been blocked!")
    except Exception as e:
        print(f"   Blocked! {type(e).__name__}: {str(e)[:70]}...")

    con.close()
    print("\n   All external access blocked successfully!")


def demo_disabled_filesystems():
    """Demonstrate disabling specific filesystems."""
    print("\n" + "=" * 60)
    print("Demo 2: Disable Specific Filesystems")
    print("=" * 60)

    # Disable local filesystem - must use SET after connection
    con = duckdb.connect(":memory:")
    con.execute("SET disabled_filesystems = 'LocalFileSystem'")

    print("\n   Disabled LocalFileSystem via SET statement...")

    print("\n   Attempting to read local file...")
    try:
        con.execute("SELECT * FROM read_csv('/etc/passwd')")
        print("   ERROR: Should have been blocked!")
    except Exception as e:
        print(f"   Blocked! {type(e).__name__}: {str(e)[:70]}...")

    con.close()


def demo_allowed_directories():
    """Demonstrate allowed_directories for fine-grained access."""
    print("\n" + "=" * 60)
    print("Demo 3: Allowed Directories (Allowlist Approach)")
    print("=" * 60)

    with tempfile.TemporaryDirectory() as allowed_dir:
        # Create a test file in the allowed directory
        allowed_file = os.path.join(allowed_dir, "data.csv")
        with open(allowed_file, "w") as f:
            f.write("id,name\n1,Alice\n2,Bob\n")

        # Create a file outside the allowed directory
        other_dir = tempfile.mkdtemp()
        blocked_file = os.path.join(other_dir, "secret.csv")
        with open(blocked_file, "w") as f:
            f.write("secret,data\n1,password\n")

        print(f"\n   Allowed directory: {allowed_dir}")
        print(f"   Blocked directory: {other_dir}")

        # Configure with allowed_directories - need to use SET for array values
        con = duckdb.connect(":memory:")
        con.execute(f"SET allowed_directories = ['{allowed_dir}']")
        con.execute("SET enable_external_access = false")

        # Try to read from allowed directory
        print("\n   Reading from ALLOWED directory...")
        try:
            result = con.execute(f"SELECT * FROM read_csv('{allowed_file}')").fetchall()
            print(f"   Success! Read {len(result)} rows from allowed file")
        except Exception as e:
            print(f"   Error: {e}")

        # Try to read from blocked directory
        print("\n   Attempting to read from BLOCKED directory...")
        try:
            con.execute(f"SELECT * FROM read_csv('{blocked_file}')")
            print("   ERROR: Should have been blocked!")
        except Exception as e:
            print(f"   Blocked! {type(e).__name__}: {str(e)[:70]}...")

        # Try to read system file
        print("\n   Attempting to read system file (/etc/passwd)...")
        try:
            con.execute("SELECT * FROM read_csv('/etc/passwd')")
            print("   ERROR: Should have been blocked!")
        except Exception as e:
            print(f"   Blocked! {type(e).__name__}: {str(e)[:70]}...")

        con.close()
        os.remove(blocked_file)
        os.rmdir(other_dir)


def demo_allowed_paths():
    """Demonstrate allowed_paths for specific file access."""
    print("\n" + "=" * 60)
    print("Demo 4: Allowed Paths (Specific File Allowlist)")
    print("=" * 60)

    with tempfile.TemporaryDirectory() as tmpdir:
        # Create test files
        allowed_file = os.path.join(tmpdir, "allowed.csv")
        blocked_file = os.path.join(tmpdir, "blocked.csv")

        with open(allowed_file, "w") as f:
            f.write("x\n1\n2\n3\n")
        with open(blocked_file, "w") as f:
            f.write("secret\npassword\n")

        print(f"\n   Allowed file: {allowed_file}")
        print(f"   Blocked file: {blocked_file}")

        # Use SET for array values
        con = duckdb.connect(":memory:")
        con.execute(f"SET allowed_paths = ['{allowed_file}']")
        con.execute("SET enable_external_access = false")

        print("\n   Reading ALLOWED file...")
        try:
            result = con.execute(f"SELECT * FROM read_csv('{allowed_file}')").fetchall()
            print(f"   Success! Read {len(result)} rows")
        except Exception as e:
            print(f"   Error: {e}")

        print("\n   Attempting to read BLOCKED file (same directory!)...")
        try:
            con.execute(f"SELECT * FROM read_csv('{blocked_file}')")
            print("   ERROR: Should have been blocked!")
        except Exception as e:
            print(f"   Blocked! {type(e).__name__}: {str(e)[:70]}...")

        con.close()


def demo_lock_configuration():
    """Demonstrate locking configuration to prevent changes."""
    print("\n" + "=" * 60)
    print("Demo 5: Lock Configuration (Prevent Setting Changes)")
    print("=" * 60)

    con = duckdb.connect(":memory:", config={
        "enable_external_access": "false",
        "lock_configuration": "true"
    })

    print("\n   Configuration locked with enable_external_access=false")

    # Try to re-enable external access
    print("\n   Attempting to SET enable_external_access = true...")
    try:
        con.execute("SET enable_external_access = true")
        print("   ERROR: Should have been blocked!")
    except Exception as e:
        print(f"   Blocked! {type(e).__name__}: {str(e)[:70]}...")

    # Try to unlock configuration
    print("\n   Attempting to SET lock_configuration = false...")
    try:
        con.execute("SET lock_configuration = false")
        print("   ERROR: Should have been blocked!")
    except Exception as e:
        print(f"   Blocked! {type(e).__name__}: {str(e)[:70]}...")

    con.close()
    print("\n   Configuration cannot be modified by untrusted queries!")


def demo_disable_http():
    """Demonstrate disabling HTTP/HTTPS access."""
    print("\n" + "=" * 60)
    print("Demo 6: Disable HTTP/HTTPS Access")
    print("=" * 60)

    # Disable httpfs filesystem - must use SET after connection
    con = duckdb.connect(":memory:")
    con.execute("SET disabled_filesystems = 'HTTPFileSystem'")

    print("\n   Disabled HTTPFileSystem via SET statement...")

    print("\n   Attempting to read from HTTPS URL...")
    try:
        con.execute("SELECT * FROM read_csv('https://example.com/data.csv')")
        print("   ERROR: Should have been blocked!")
    except Exception as e:
        print(f"   Blocked! {type(e).__name__}: {str(e)[:70]}...")

    con.close()


def main():
    print("=" * 60)
    print("DuckDB File/Network Access Restrictions Demo")
    print("=" * 60)

    demo_disable_external_access()
    demo_disabled_filesystems()
    demo_allowed_directories()
    demo_allowed_paths()
    demo_lock_configuration()
    demo_disable_http()

    print("\n" + "=" * 60)
    print("FILE/NETWORK RESTRICTIONS DEMO COMPLETE!")
    print("=" * 60)

if __name__ == "__main__":
    main()
