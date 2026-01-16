#!/usr/bin/env python3
"""
Demonstration of query timeout/time limit implementation for DuckDB.

DuckDB does not have a native query timeout setting, but we can implement
timeouts using the interrupt() method called from a separate thread.
"""
import duckdb
import threading
import time
from concurrent.futures import ThreadPoolExecutor, TimeoutError as FuturesTimeoutError


def query_with_timeout_v1(con, query, timeout_seconds):
    """
    Execute a query with a timeout using threading.Timer.

    This approach uses a timer thread to call con.interrupt() after the timeout.
    """
    result = None
    exception = None
    timed_out = False

    def interrupt_query():
        nonlocal timed_out
        timed_out = True
        con.interrupt()

    # Set up timer to interrupt query after timeout
    timer = threading.Timer(timeout_seconds, interrupt_query)
    timer.start()

    try:
        result = con.execute(query).fetchall()
    except duckdb.InterruptException:
        exception = TimeoutError(f"Query exceeded {timeout_seconds}s timeout")
    except Exception as e:
        exception = e
    finally:
        timer.cancel()

    if exception:
        raise exception
    return result


def query_with_timeout_v2(con, query, timeout_seconds):
    """
    Execute a query with a timeout using ThreadPoolExecutor.

    This approach runs the query in a thread pool and uses the timeout
    parameter of the future.result() method.
    """
    def run_query():
        return con.execute(query).fetchall()

    with ThreadPoolExecutor(max_workers=1) as executor:
        future = executor.submit(run_query)

        # Set up interrupt in case of timeout
        timer = threading.Timer(timeout_seconds, con.interrupt)
        timer.start()

        try:
            result = future.result(timeout=timeout_seconds)
            timer.cancel()
            return result
        except FuturesTimeoutError:
            timer.cancel()  # Cancel timer if future times out first
            raise TimeoutError(f"Query exceeded {timeout_seconds}s timeout")
        except duckdb.InterruptException:
            timer.cancel()
            raise TimeoutError(f"Query exceeded {timeout_seconds}s timeout")


class TimeoutConnection:
    """
    A wrapper around DuckDB connection that enforces query timeouts.

    Usage:
        con = TimeoutConnection(duckdb.connect(":memory:"), timeout_ms=500)
        con.execute("SELECT * FROM large_table")
    """

    def __init__(self, connection, timeout_ms=500):
        self._con = connection
        self._timeout_seconds = timeout_ms / 1000.0

    def execute(self, query, parameters=None):
        """Execute query with timeout enforcement."""
        result = None
        exception = None
        timed_out = False

        def interrupt_query():
            nonlocal timed_out
            timed_out = True
            self._con.interrupt()

        timer = threading.Timer(self._timeout_seconds, interrupt_query)
        timer.start()

        try:
            if parameters:
                result = self._con.execute(query, parameters)
            else:
                result = self._con.execute(query)
        except duckdb.InterruptException:
            exception = TimeoutError(
                f"Query exceeded {self._timeout_seconds * 1000}ms timeout"
            )
        except Exception as e:
            exception = e
        finally:
            timer.cancel()

        if exception:
            raise exception
        return result

    def close(self):
        self._con.close()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()


def demo_basic_timeout():
    """Demonstrate basic timeout with a slow query."""
    print("\n" + "=" * 60)
    print("Demo 1: Basic Query Timeout")
    print("=" * 60)

    con = duckdb.connect(":memory:")

    # Create a table and test quick query
    con.execute("CREATE TABLE test AS SELECT range AS x FROM range(1000)")

    print("\n   Testing fast query (should complete)...")
    start = time.time()
    try:
        result = query_with_timeout_v1(con, "SELECT COUNT(*) FROM test", 1.0)
        elapsed = time.time() - start
        print(f"   Success! Result: {result[0][0]} rows, took {elapsed:.3f}s")
    except TimeoutError as e:
        print(f"   Unexpected timeout: {e}")

    # Now test with artificially slow query
    print("\n   Testing slow query (should timeout after 500ms)...")

    # Create a cross join that will be slow
    con.execute("CREATE TABLE slow_data AS SELECT range AS x FROM range(10000)")

    start = time.time()
    try:
        # This query creates a huge cross product - will be slow
        result = query_with_timeout_v1(
            con,
            "SELECT COUNT(*) FROM slow_data a, slow_data b, slow_data c",
            0.5  # 500ms timeout
        )
        elapsed = time.time() - start
        print(f"   Query completed in {elapsed:.3f}s - result: {result}")
    except TimeoutError as e:
        elapsed = time.time() - start
        print(f"   Timed out after {elapsed:.3f}s (expected!)")

    con.close()


def demo_timeout_wrapper():
    """Demonstrate the TimeoutConnection wrapper class."""
    print("\n" + "=" * 60)
    print("Demo 2: TimeoutConnection Wrapper Class")
    print("=" * 60)

    # Create a connection with 500ms timeout
    print("\n   Creating TimeoutConnection with 500ms timeout...")

    with TimeoutConnection(duckdb.connect(":memory:"), timeout_ms=500) as con:
        # Setup test data
        con.execute("CREATE TABLE users (id INT, name VARCHAR)")
        con.execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob')")

        print("\n   Fast query (should complete)...")
        start = time.time()
        try:
            result = con.execute("SELECT * FROM users").fetchall()
            elapsed = time.time() - start
            print(f"   Success! Got {len(result)} rows in {elapsed:.3f}s")
        except TimeoutError as e:
            print(f"   Error: {e}")

        # Create large dataset for slow query
        con.execute("CREATE TABLE big AS SELECT range x FROM range(10000)")

        print("\n   Slow query (cross join - should timeout)...")
        start = time.time()
        try:
            result = con.execute(
                "SELECT COUNT(*) FROM big a, big b, big c"
            ).fetchall()
            elapsed = time.time() - start
            print(f"   Completed in {elapsed:.3f}s - result: {result}")
        except TimeoutError as e:
            elapsed = time.time() - start
            print(f"   Timed out after {elapsed:.3f}s (expected!)")


def demo_resource_limits():
    """Demonstrate other resource limit options."""
    print("\n" + "=" * 60)
    print("Demo 3: Additional Resource Limits")
    print("=" * 60)

    con = duckdb.connect(":memory:")

    # Thread limit
    print("\n   Setting thread limit to 1...")
    con.execute("SET threads = 1")
    result = con.execute("SELECT current_setting('threads')").fetchone()
    print(f"   Threads now set to: {result[0]}")

    # Memory limit
    print("\n   Setting memory limit to 100MB...")
    con.execute("SET memory_limit = '100MB'")
    result = con.execute("SELECT current_setting('memory_limit')").fetchone()
    print(f"   Memory limit now set to: {result[0]}")

    # Temp directory size limit
    print("\n   Setting temp directory size limit to 50MB...")
    con.execute("SET max_temp_directory_size = '50MB'")
    result = con.execute("SELECT current_setting('max_temp_directory_size')").fetchone()
    print(f"   Max temp directory size: {result[0]}")

    con.close()

    print("\n   Resource limits configured successfully!")


def demo_combined_secure_connection():
    """Demonstrate combining all security measures."""
    print("\n" + "=" * 60)
    print("Demo 4: Combined Secure Connection")
    print("=" * 60)

    print("\n   Creating maximally secure sandboxed connection...")

    # Start with basic connection
    inner_con = duckdb.connect(":memory:")

    # Apply security settings BEFORE locking
    inner_con.execute("SET enable_external_access = false")
    inner_con.execute("SET threads = 2")
    inner_con.execute("SET memory_limit = '256MB'")
    inner_con.execute("SET max_temp_directory_size = '100MB'")

    # Lock configuration to prevent untrusted queries from changing settings
    inner_con.execute("SET lock_configuration = true")

    # Wrap with timeout enforcement
    con = TimeoutConnection(inner_con, timeout_ms=500)

    print("   Security settings applied:")
    print("   - enable_external_access: false")
    print("   - threads: 2")
    print("   - memory_limit: 256MB")
    print("   - max_temp_directory_size: 100MB")
    print("   - lock_configuration: true")
    print("   - query timeout: 500ms")

    # Test that external access is blocked
    print("\n   Testing external access restriction...")
    try:
        con.execute("SELECT * FROM read_csv('/etc/passwd')")
        print("   ERROR: Should have been blocked!")
    except Exception as e:
        print(f"   Blocked! {type(e).__name__}")

    # Test that configuration cannot be changed
    print("\n   Testing configuration lock...")
    try:
        con.execute("SET enable_external_access = true")
        print("   ERROR: Should have been blocked!")
    except Exception as e:
        print(f"   Blocked! {type(e).__name__}")

    # Test that quick queries work
    print("\n   Testing quick query...")
    con.execute("CREATE TABLE test (x INT)")
    con.execute("INSERT INTO test VALUES (1), (2), (3)")
    result = con.execute("SELECT SUM(x) FROM test").fetchone()
    print(f"   Query result: {result[0]}")

    con.close()
    print("\n   Secure sandboxed execution successful!")


def main():
    print("=" * 60)
    print("DuckDB Query Timeout/Time Limit Demo")
    print("=" * 60)

    demo_basic_timeout()
    demo_timeout_wrapper()
    demo_resource_limits()
    demo_combined_secure_connection()

    print("\n" + "=" * 60)
    print("QUERY TIMEOUT DEMO COMPLETE!")
    print("=" * 60)


if __name__ == "__main__":
    main()
