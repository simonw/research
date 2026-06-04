#!/usr/bin/env python3
"""
Sandboxed DuckDB executor for running untrusted queries safely.

This module provides a secure wrapper around DuckDB connections that:
1. Enforces read-only access (optional)
2. Prevents access to files outside allowed paths
3. Blocks HTTP/HTTPS requests
4. Enforces query timeouts
5. Limits resource usage (memory, threads, temp storage)
6. Locks configuration to prevent tampering

Example usage:
    executor = SandboxedDuckDB(
        allowed_paths=["/data/public.parquet"],
        timeout_ms=500,
        memory_limit="256MB"
    )

    # Safe to run untrusted queries
    result = executor.execute("SELECT * FROM read_parquet('/data/public.parquet')")
"""
import duckdb
import threading
from dataclasses import dataclass, field
from typing import Any


class QueryTimeoutError(Exception):
    """Raised when a query exceeds the configured timeout."""
    pass


@dataclass
class SandboxConfig:
    """Configuration for sandboxed DuckDB execution."""

    # File access
    allowed_directories: list[str] = field(default_factory=list)
    allowed_paths: list[str] = field(default_factory=list)
    enable_external_access: bool = False

    # Query limits
    timeout_ms: int = 500  # Default 500ms timeout
    memory_limit: str = "256MB"
    max_threads: int = 2
    max_temp_directory_size: str = "100MB"

    # Access mode
    read_only: bool = False  # Set True for read-only mode on existing DB


class SandboxedDuckDB:
    """
    A sandboxed DuckDB connection for safely executing untrusted queries.

    The connection is configured with security restrictions that cannot be
    changed by the queries being executed.
    """

    def __init__(
        self,
        database: str = ":memory:",
        allowed_directories: list[str] | None = None,
        allowed_paths: list[str] | None = None,
        timeout_ms: int = 500,
        memory_limit: str = "256MB",
        max_threads: int = 2,
        max_temp_directory_size: str = "100MB",
        read_only: bool = False,
    ):
        """
        Create a sandboxed DuckDB connection.

        Args:
            database: Database path or ":memory:" for in-memory
            allowed_directories: List of directories to allow file access
            allowed_paths: List of specific files to allow access
            timeout_ms: Query timeout in milliseconds (default 500ms)
            memory_limit: Maximum memory usage (e.g., "256MB")
            max_threads: Maximum number of threads for query execution
            max_temp_directory_size: Maximum size for temporary files
            read_only: Whether to open existing database in read-only mode
        """
        self.timeout_seconds = timeout_ms / 1000.0
        self._lock = threading.Lock()

        # Connect with optional read-only mode
        if read_only and database != ":memory:":
            self._con = duckdb.connect(database, read_only=True)
        else:
            self._con = duckdb.connect(database)

        # Configure allowed paths BEFORE disabling external access
        if allowed_directories:
            dirs_sql = "[" + ",".join(f"'{d}'" for d in allowed_directories) + "]"
            self._con.execute(f"SET allowed_directories = {dirs_sql}")

        if allowed_paths:
            paths_sql = "[" + ",".join(f"'{p}'" for p in allowed_paths) + "]"
            self._con.execute(f"SET allowed_paths = {paths_sql}")

        # Disable external access (allowed paths still work)
        self._con.execute("SET enable_external_access = false")

        # Set resource limits
        self._con.execute(f"SET threads = {max_threads}")
        self._con.execute(f"SET memory_limit = '{memory_limit}'")
        self._con.execute(f"SET max_temp_directory_size = '{max_temp_directory_size}'")

        # Lock configuration to prevent untrusted queries from changing settings
        self._con.execute("SET lock_configuration = true")

    def execute(self, query: str, parameters: list | None = None) -> duckdb.DuckDBPyRelation:
        """
        Execute a query with timeout enforcement.

        Args:
            query: SQL query to execute
            parameters: Optional query parameters

        Returns:
            DuckDB relation/result

        Raises:
            QueryTimeoutError: If query exceeds timeout
            duckdb.Error: For other DuckDB errors
        """
        result = None
        exception = None
        timed_out = False

        def interrupt_query():
            nonlocal timed_out
            timed_out = True
            self._con.interrupt()

        with self._lock:
            timer = threading.Timer(self.timeout_seconds, interrupt_query)
            timer.start()

            try:
                if parameters:
                    result = self._con.execute(query, parameters)
                else:
                    result = self._con.execute(query)
            except duckdb.InterruptException:
                exception = QueryTimeoutError(
                    f"Query exceeded {self.timeout_seconds * 1000}ms timeout"
                )
            except Exception as e:
                exception = e
            finally:
                timer.cancel()

        if exception:
            raise exception
        return result

    def fetchall(self, query: str, parameters: list | None = None) -> list[tuple]:
        """Execute query and fetch all results."""
        return self.execute(query, parameters).fetchall()

    def fetchone(self, query: str, parameters: list | None = None) -> tuple | None:
        """Execute query and fetch one result."""
        return self.execute(query, parameters).fetchone()

    def fetchdf(self, query: str, parameters: list | None = None) -> Any:
        """Execute query and return pandas DataFrame."""
        return self.execute(query, parameters).fetchdf()

    def close(self):
        """Close the connection."""
        self._con.close()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()


def demo():
    """Demonstrate the sandboxed executor."""
    import tempfile
    import os

    print("=" * 60)
    print("SandboxedDuckDB Demo")
    print("=" * 60)

    with tempfile.TemporaryDirectory() as tmpdir:
        # Create test data file
        data_file = os.path.join(tmpdir, "data.csv")
        with open(data_file, "w") as f:
            f.write("id,name,value\n1,Alice,100\n2,Bob,200\n3,Charlie,300\n")

        secret_file = os.path.join(tmpdir, "secret.csv")
        with open(secret_file, "w") as f:
            f.write("password\nhunter2\n")

        print(f"\nAllowed file: {data_file}")
        print(f"Blocked file: {secret_file}")

        # Create sandboxed executor with specific file access
        with SandboxedDuckDB(
            allowed_paths=[data_file],
            timeout_ms=500,
            memory_limit="128MB",
            max_threads=1
        ) as db:
            # Test 1: Read allowed file
            print("\n1. Reading allowed file...")
            result = db.fetchall(f"SELECT * FROM read_csv('{data_file}')")
            print(f"   Success! Got {len(result)} rows: {result}")

            # Test 2: Try to read blocked file
            print("\n2. Trying to read blocked file...")
            try:
                db.fetchall(f"SELECT * FROM read_csv('{secret_file}')")
                print("   ERROR: Should have been blocked!")
            except Exception as e:
                print(f"   Blocked! {type(e).__name__}")

            # Test 3: Try to read system file
            print("\n3. Trying to read /etc/passwd...")
            try:
                db.fetchall("SELECT * FROM read_csv('/etc/passwd')")
                print("   ERROR: Should have been blocked!")
            except Exception as e:
                print(f"   Blocked! {type(e).__name__}")

            # Test 4: Try to change configuration
            print("\n4. Trying to change security settings...")
            try:
                db.execute("SET enable_external_access = true")
                print("   ERROR: Should have been blocked!")
            except Exception as e:
                print(f"   Blocked! {type(e).__name__}")

            # Test 5: Test timeout with slow query
            print("\n5. Testing query timeout (500ms)...")
            db.execute("CREATE TABLE big AS SELECT range x FROM range(5000)")
            try:
                db.fetchall("SELECT COUNT(*) FROM big a, big b, big c")
                print("   Query completed (was fast enough)")
            except QueryTimeoutError as e:
                print(f"   Timed out! {e}")

    print("\n" + "=" * 60)
    print("Demo complete - all security constraints enforced!")
    print("=" * 60)


if __name__ == "__main__":
    demo()
