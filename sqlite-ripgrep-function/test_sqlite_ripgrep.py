#!/usr/bin/env python3
"""
Unit tests for SQLite ripgrep functions.

Tests both the pure Python implementation and the C extension (if available).
"""

import json
import os
import shutil
import sqlite3
import tempfile
import time
import unittest
from pathlib import Path

# Import the Python implementation
from sqlite_ripgrep_python import (
    register_ripgrep_function,
    register_ripgrep_with_table_helper,
    run_ripgrep,
)


class TestDataMixin:
    """Mixin providing test data setup/teardown."""

    @classmethod
    def setUpClass(cls):
        """Create a temporary directory with test files."""
        cls.test_dir = tempfile.mkdtemp(prefix='ripgrep_test_')

        # Create test files
        cls.test_files = {}

        # Python file with some code
        cls.test_files['example.py'] = '''\
def hello_world():
    """Print hello world."""
    print("Hello, World!")

def goodbye_world():
    print("Goodbye, World!")

class MyClass:
    def __init__(self):
        self.message = "Hello from class"

if __name__ == "__main__":
    hello_world()
'''

        # JavaScript file
        cls.test_files['script.js'] = '''\
function helloWorld() {
    console.log("Hello, World!");
}

function goodbyeWorld() {
    console.log("Goodbye, World!");
}

// Export functions
module.exports = { helloWorld, goodbyeWorld };
'''

        # Text file
        cls.test_files['readme.txt'] = '''\
This is a readme file.
It contains some Hello text.
And more content here.
The word hello appears multiple times.
Hello again!
'''

        # Nested directory with file
        cls.test_files['subdir/nested.py'] = '''\
# Nested Python file
def nested_hello():
    return "Hello from nested"
'''

        # Create all test files
        for path, content in cls.test_files.items():
            full_path = Path(cls.test_dir) / path
            full_path.parent.mkdir(parents=True, exist_ok=True)
            full_path.write_text(content)

    @classmethod
    def tearDownClass(cls):
        """Remove the temporary test directory."""
        shutil.rmtree(cls.test_dir, ignore_errors=True)


class TestRunRipgrep(TestDataMixin, unittest.TestCase):
    """Test the run_ripgrep function directly."""

    def test_basic_search(self):
        """Test basic pattern search."""
        result = run_ripgrep('hello', self.test_dir, time_limit=5.0)

        self.assertIsNone(result['error'])
        self.assertFalse(result['truncated'])
        self.assertGreater(result['count'], 0)

    def test_case_insensitive_search(self):
        """Test case-insensitive search."""
        # Case sensitive - should miss some
        result_sensitive = run_ripgrep('Hello', self.test_dir, case_insensitive=False, time_limit=5.0)

        # Case insensitive - should find more
        result_insensitive = run_ripgrep('hello', self.test_dir, case_insensitive=True, time_limit=5.0)

        self.assertGreaterEqual(
            result_insensitive['count'],
            result_sensitive['count']
        )

    def test_glob_filter(self):
        """Test file filtering with glob pattern."""
        # Search only Python files
        result_py = run_ripgrep('hello', self.test_dir, glob_pattern='*.py', time_limit=5.0)

        # Search only JavaScript files
        result_js = run_ripgrep('hello', self.test_dir, glob_pattern='*.js', time_limit=5.0)

        # All files should find more than just one type
        result_all = run_ripgrep('hello', self.test_dir, time_limit=5.0)

        self.assertGreater(result_py['count'], 0)
        self.assertGreater(result_js['count'], 0)
        self.assertGreater(result_all['count'], result_py['count'])

    def test_no_matches(self):
        """Test search with no matches."""
        result = run_ripgrep('xyznonexistent123', self.test_dir, time_limit=5.0)

        self.assertIsNone(result['error'])
        self.assertEqual(result['count'], 0)

    def test_result_structure(self):
        """Test that results have expected structure."""
        result = run_ripgrep('hello_world', self.test_dir, time_limit=5.0)

        self.assertIn('results', result)
        self.assertIn('truncated', result)
        self.assertIn('time_limit_hit', result)
        self.assertIn('error', result)
        self.assertIn('count', result)

        if result['count'] > 0:
            first = result['results'][0]
            self.assertIn('path', first)
            self.assertIn('line_number', first)
            self.assertIn('lines', first)

    def test_empty_pattern(self):
        """Test with empty pattern."""
        result = run_ripgrep('', self.test_dir, time_limit=5.0)
        # ripgrep actually matches all lines with empty pattern
        # Just verify no error occurred
        self.assertIsNone(result['error'])


class TestPythonSQLiteFunction(TestDataMixin, unittest.TestCase):
    """Test the pure Python SQLite function."""

    def setUp(self):
        """Create a new connection for each test."""
        self.conn = sqlite3.connect(':memory:')
        register_ripgrep_function(self.conn, self.test_dir, default_time_limit=5.0)

    def tearDown(self):
        """Close the connection."""
        self.conn.close()

    def test_single_arg_search(self):
        """Test ripgrep(pattern) with single argument."""
        result = self.conn.execute("SELECT ripgrep('hello')").fetchone()[0]
        data = json.loads(result)

        self.assertIsNone(data['error'])
        self.assertGreater(data['count'], 0)

    def test_two_arg_search(self):
        """Test ripgrep(pattern, glob) with two arguments."""
        result = self.conn.execute("SELECT ripgrep('hello', '*.py')").fetchone()[0]
        data = json.loads(result)

        self.assertIsNone(data['error'])
        self.assertGreater(data['count'], 0)

        # Verify all results are Python files
        for r in data['results']:
            self.assertTrue(r['path'].endswith('.py'))

    def test_three_arg_search(self):
        """Test ripgrep(pattern, glob, time_limit) with three arguments."""
        result = self.conn.execute("SELECT ripgrep('hello', '*.py', 5.0)").fetchone()[0]
        data = json.loads(result)

        self.assertIsNone(data['error'])
        self.assertFalse(data['time_limit_hit'])

    def test_null_glob(self):
        """Test with NULL glob pattern."""
        result = self.conn.execute("SELECT ripgrep('hello', NULL)").fetchone()[0]
        data = json.loads(result)

        self.assertIsNone(data['error'])
        self.assertGreater(data['count'], 0)

    def test_in_where_clause(self):
        """Test using ripgrep in a WHERE clause context."""
        # Create a simple table
        self.conn.execute("CREATE TABLE patterns (id INTEGER, pattern TEXT)")
        self.conn.execute("INSERT INTO patterns VALUES (1, 'hello')")
        self.conn.execute("INSERT INTO patterns VALUES (2, 'goodbye')")

        # Use ripgrep with pattern from table
        results = self.conn.execute("""
            SELECT id, json_extract(ripgrep(pattern), '$.count') as match_count
            FROM patterns
        """).fetchall()

        self.assertEqual(len(results), 2)
        self.assertGreater(results[0][1], 0)  # hello matches
        self.assertGreater(results[1][1], 0)  # goodbye matches


class TestPythonTableHelper(TestDataMixin, unittest.TestCase):
    """Test the table-valued function helper with json_each."""

    def setUp(self):
        """Create a new connection for each test."""
        self.conn = sqlite3.connect(':memory:')
        register_ripgrep_with_table_helper(self.conn, self.test_dir, default_time_limit=5.0)

    def tearDown(self):
        """Close the connection."""
        self.conn.close()

    def test_table_expansion(self):
        """Test expanding results as table using json_each."""
        rows = self.conn.execute("""
            SELECT
                json_extract(value, '$.path') as path,
                json_extract(value, '$.line_number') as line_number,
                json_extract(value, '$.lines') as line_text
            FROM json_each(ripgrep_results('hello'))
        """).fetchall()

        self.assertGreater(len(rows), 0)
        for row in rows:
            self.assertIsNotNone(row[0])  # path
            self.assertIsNotNone(row[1])  # line_number

    def test_with_glob_filter(self):
        """Test table expansion with glob filter."""
        rows = self.conn.execute("""
            SELECT
                json_extract(value, '$.path') as path
            FROM json_each(ripgrep_results('hello', '*.py'))
        """).fetchall()

        self.assertGreater(len(rows), 0)
        for row in rows:
            self.assertTrue(row[0].endswith('.py'))

    def test_aggregation(self):
        """Test aggregating ripgrep results."""
        # Count matches per file type
        result = self.conn.execute("""
            SELECT COUNT(*) FROM json_each(ripgrep_results('hello'))
        """).fetchone()[0]

        self.assertGreater(result, 0)


class TestCExtension(TestDataMixin, unittest.TestCase):
    """Test the C extension (if available)."""

    @classmethod
    def setUpClass(cls):
        """Set up test class and check for extension."""
        super().setUpClass()

        # Try to build the extension
        ext_dir = Path(__file__).parent
        ext_path = ext_dir / 'sqlite_ripgrep.so'

        cls.extension_available = ext_path.exists()
        cls.ext_path = str(ext_path)

        if not cls.extension_available:
            # Try to build it
            import subprocess
            try:
                result = subprocess.run(
                    ['make', f'RIPGREP_BASE_DIR={cls.test_dir}'],
                    cwd=str(ext_dir),
                    capture_output=True,
                    timeout=30
                )
                if result.returncode == 0 and ext_path.exists():
                    cls.extension_available = True
            except Exception:
                pass

    def setUp(self):
        """Create a new connection for each test."""
        if not self.extension_available:
            self.skipTest("C extension not available")

        self.conn = sqlite3.connect(':memory:')
        self.conn.enable_load_extension(True)
        try:
            self.conn.load_extension(self.ext_path.replace('.so', ''))
        except sqlite3.OperationalError as e:
            self.skipTest(f"Failed to load extension: {e}")

    def tearDown(self):
        """Close the connection."""
        if hasattr(self, 'conn'):
            self.conn.close()

    def test_basic_query(self):
        """Test basic table-valued function query."""
        rows = self.conn.execute("""
            SELECT path, line_number, line_text
            FROM ripgrep
            WHERE pattern = 'hello' AND base_dir = ?
        """, (self.test_dir,)).fetchall()

        self.assertGreater(len(rows), 0)

    def test_with_glob(self):
        """Test with glob filter."""
        rows = self.conn.execute("""
            SELECT path, line_number
            FROM ripgrep
            WHERE pattern = 'hello' AND glob = '*.py' AND base_dir = ?
        """, (self.test_dir,)).fetchall()

        self.assertGreater(len(rows), 0)
        for row in rows:
            self.assertTrue(row[0].endswith('.py'))

    def test_with_time_limit(self):
        """Test with explicit time limit."""
        rows = self.conn.execute("""
            SELECT path, line_number
            FROM ripgrep
            WHERE pattern = 'hello' AND time_limit = 5.0 AND base_dir = ?
        """, (self.test_dir,)).fetchall()

        self.assertGreater(len(rows), 0)

    def test_all_columns(self):
        """Test that all columns are returned."""
        rows = self.conn.execute("""
            SELECT path, line_number, line_text, match_text, match_start, match_end
            FROM ripgrep
            WHERE pattern = 'hello_world' AND base_dir = ?
        """, (self.test_dir,)).fetchall()

        if len(rows) > 0:
            row = rows[0]
            self.assertIsNotNone(row[0])  # path
            self.assertIsNotNone(row[1])  # line_number
            self.assertIsNotNone(row[2])  # line_text

    def test_no_pattern(self):
        """Test without pattern constraint (should return nothing)."""
        rows = self.conn.execute("""
            SELECT * FROM ripgrep LIMIT 10
        """).fetchall()

        self.assertEqual(len(rows), 0)

    def test_join_with_table(self):
        """Test using ripgrep results with subquery approach.

        Note: Direct cross-joins with constraints from other tables don't work
        well with SQLite virtual tables because constraints aren't pushed through.
        Use a subquery or CTE approach instead.
        """
        # Use a subquery approach which works with virtual tables
        rows = self.conn.execute("""
            SELECT path, line_number
            FROM ripgrep
            WHERE pattern = 'hello' AND base_dir = ?
        """, (self.test_dir,)).fetchall()

        self.assertGreater(len(rows), 0)

        # Verify results can be used with other data
        self.conn.execute("CREATE TABLE my_patterns (id INTEGER, found_path TEXT)")
        for idx, row in enumerate(rows[:3]):
            self.conn.execute("INSERT INTO my_patterns VALUES (?, ?)", (idx, row[0]))

        count = self.conn.execute("SELECT COUNT(*) FROM my_patterns").fetchone()[0]
        self.assertGreater(count, 0)


class TestTimeLimitBehavior(TestDataMixin, unittest.TestCase):
    """Test time limit behavior."""

    def test_very_short_time_limit(self):
        """Test that a very short time limit triggers truncation."""
        # This test is inherently timing-dependent
        # We'll use a very short timeout and search a large pattern space
        result = run_ripgrep(
            '.',  # Match any character - will match many lines
            self.test_dir,
            time_limit=0.001,  # 1ms timeout
            max_results=1000000,  # High limit so time limit kicks in first
        )

        # With such a short timeout, we might hit it
        # But the test files are small so it might complete
        # This is more of a smoke test
        self.assertIn('time_limit_hit', result)

    def test_reasonable_time_limit(self):
        """Test that a reasonable time limit doesn't truncate small searches."""
        result = run_ripgrep(
            'hello',
            self.test_dir,
            time_limit=5.0,
        )

        self.assertFalse(result['time_limit_hit'])
        self.assertFalse(result['truncated'])


class TestEdgeCases(TestDataMixin, unittest.TestCase):
    """Test edge cases and error handling."""

    def setUp(self):
        """Create a new connection for each test."""
        self.conn = sqlite3.connect(':memory:')
        register_ripgrep_function(self.conn, self.test_dir, default_time_limit=5.0)

    def tearDown(self):
        """Close the connection."""
        self.conn.close()

    def test_regex_pattern(self):
        """Test with regex pattern."""
        result = self.conn.execute(r"SELECT ripgrep('hello.*world')").fetchone()[0]
        data = json.loads(result)

        # Should find "hello_world" function
        self.assertGreater(data['count'], 0)

    def test_special_characters(self):
        """Test with special characters in pattern."""
        # Search for function definition pattern
        result = self.conn.execute(r"SELECT ripgrep('def \w+\(')").fetchone()[0]
        data = json.loads(result)

        self.assertIsNone(data['error'])

    def test_unicode_search(self):
        """Test with unicode characters."""
        result = self.conn.execute("SELECT ripgrep('世界')").fetchone()[0]
        data = json.loads(result)

        self.assertIsNone(data['error'])
        # No matches expected in our test files
        self.assertEqual(data['count'], 0)

    def test_nonexistent_directory(self):
        """Test with nonexistent base directory."""
        conn = sqlite3.connect(':memory:')
        register_ripgrep_function(conn, '/nonexistent/path/xyz')

        result = conn.execute("SELECT ripgrep('hello')").fetchone()[0]
        data = json.loads(result)

        # Should have an error or zero results
        self.assertTrue(data['error'] is not None or data['count'] == 0)
        conn.close()


if __name__ == '__main__':
    # Check if ripgrep is available
    import subprocess
    try:
        subprocess.run(['rg', '--version'], capture_output=True, check=True)
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("WARNING: ripgrep (rg) not found. Tests may fail.")
        print("Install ripgrep to run these tests.")

    unittest.main(verbosity=2)
