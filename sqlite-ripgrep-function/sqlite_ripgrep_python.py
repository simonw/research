"""
Pure Python SQLite ripgrep function.

This module provides a SQLite function that runs ripgrep searches.
It can be registered with any SQLite connection.

Usage:
    import sqlite3
    from sqlite_ripgrep_python import register_ripgrep_function

    conn = sqlite3.connect(':memory:')
    register_ripgrep_function(conn, base_directory='/path/to/search')

    # Single argument - search term only
    results = conn.execute("SELECT ripgrep('pattern')").fetchone()[0]

    # Two arguments - search term and file glob
    results = conn.execute("SELECT ripgrep('pattern', '*.py')").fetchone()[0]

    # Three arguments - search term, file glob, and time limit
    results = conn.execute("SELECT ripgrep('pattern', '*.py', 2.0)").fetchone()[0]
"""

import json
import subprocess
import signal
import os
import threading
from pathlib import Path
from typing import Optional, List, Dict, Any


class RipgrepTimeoutError(Exception):
    """Raised when ripgrep search exceeds time limit."""
    pass


def run_ripgrep(
    pattern: str,
    base_directory: str,
    glob_pattern: Optional[str] = None,
    time_limit: float = 1.0,
    max_results: int = 1000,
    context_lines: int = 0,
    case_insensitive: bool = False,
    literal: bool = False,
) -> Dict[str, Any]:
    """
    Run ripgrep and return results as a dictionary.

    Args:
        pattern: The search pattern (regex by default)
        base_directory: Directory to search in
        glob_pattern: Optional glob pattern to filter files (e.g., '*.py')
        time_limit: Maximum time in seconds (default 1.0)
        max_results: Maximum number of results to return (default 1000)
        context_lines: Number of context lines around matches
        case_insensitive: Whether to ignore case
        literal: Whether to treat pattern as literal string

    Returns:
        Dictionary with 'results', 'truncated', and 'error' keys
    """
    args = ['rg', '-e', pattern, '--json']

    if context_lines > 0:
        args.extend(['-C', str(context_lines)])
    if case_insensitive:
        args.append('-i')
    if literal:
        args.append('-F')
    if glob_pattern:
        args.extend(['--glob', glob_pattern])

    args.append(base_directory)

    results = []
    truncated = False
    time_limit_hit = False
    error = None

    try:
        proc = subprocess.Popen(
            args,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=base_directory,
        )

        def kill_process():
            nonlocal time_limit_hit
            time_limit_hit = True
            try:
                proc.kill()
            except OSError:
                pass

        # Set up timer to kill process after time limit
        timer = threading.Timer(time_limit, kill_process)
        timer.start()

        try:
            for line in proc.stdout:
                if time_limit_hit:
                    break
                try:
                    data = json.loads(line.decode('utf-8', errors='replace'))
                    if data.get('type') == 'match':
                        # Extract useful information from the match
                        match_data = data.get('data', {})
                        result = {
                            'path': match_data.get('path', {}).get('text', ''),
                            'line_number': match_data.get('line_number'),
                            'lines': match_data.get('lines', {}).get('text', ''),
                            'submatches': [
                                {
                                    'match': sm.get('match', {}).get('text', ''),
                                    'start': sm.get('start'),
                                    'end': sm.get('end'),
                                }
                                for sm in match_data.get('submatches', [])
                            ]
                        }
                        results.append(result)

                        if len(results) >= max_results:
                            truncated = True
                            break
                except json.JSONDecodeError:
                    continue
        finally:
            timer.cancel()
            try:
                proc.kill()
            except OSError:
                pass
            # Close file handles to avoid resource warnings
            if proc.stdout:
                proc.stdout.close()
            if proc.stderr:
                proc.stderr.close()
            proc.wait()

    except FileNotFoundError:
        error = "ripgrep (rg) command not found"
    except Exception as e:
        error = str(e)

    return {
        'results': results,
        'truncated': truncated or time_limit_hit,
        'time_limit_hit': time_limit_hit,
        'error': error,
        'count': len(results),
    }


def create_ripgrep_function(base_directory: str, default_time_limit: float = 1.0):
    """
    Create a ripgrep function closure configured for a specific base directory.

    Args:
        base_directory: The directory to search in
        default_time_limit: Default time limit in seconds

    Returns:
        A function suitable for registering with SQLite
    """
    base_dir = str(Path(base_directory).resolve())

    def ripgrep_func(*args):
        """
        SQLite ripgrep function with multiple arity:
        - ripgrep(pattern) - search for pattern
        - ripgrep(pattern, glob) - search with file filter
        - ripgrep(pattern, glob, time_limit) - search with time limit

        Returns JSON string with results.
        """
        if len(args) < 1:
            return json.dumps({'error': 'At least one argument (pattern) required'})

        pattern = args[0]
        glob_pattern = args[1] if len(args) > 1 else None
        time_limit = float(args[2]) if len(args) > 2 else default_time_limit

        if not pattern:
            return json.dumps({'error': 'Pattern cannot be empty'})

        result = run_ripgrep(
            pattern=pattern,
            base_directory=base_dir,
            glob_pattern=glob_pattern,
            time_limit=time_limit,
        )

        return json.dumps(result)

    return ripgrep_func


def register_ripgrep_function(
    conn,
    base_directory: str,
    function_name: str = 'ripgrep',
    default_time_limit: float = 1.0,
):
    """
    Register the ripgrep function with a SQLite connection.

    Args:
        conn: SQLite connection object
        base_directory: Directory to search in
        function_name: Name for the SQL function (default 'ripgrep')
        default_time_limit: Default time limit in seconds
    """
    ripgrep_func = create_ripgrep_function(base_directory, default_time_limit)

    # Register with -1 for nargs to accept variable number of arguments
    conn.create_function(function_name, -1, ripgrep_func)


# Table-valued function implementation using a workaround
# Standard sqlite3 doesn't support table-valued functions directly,
# but we can use json_each() to expand the JSON results

def register_ripgrep_with_table_helper(
    conn,
    base_directory: str,
    function_name: str = 'ripgrep',
    default_time_limit: float = 1.0,
):
    """
    Register ripgrep function and create a view to help expand results.

    Usage with json_each:
        SELECT
            json_extract(value, '$.path') as path,
            json_extract(value, '$.line_number') as line_number,
            json_extract(value, '$.lines') as lines
        FROM json_each(
            json_extract(ripgrep('pattern', '*.py'), '$.results')
        )
    """
    register_ripgrep_function(conn, base_directory, function_name, default_time_limit)

    # Create a helper function to just return the results array
    def ripgrep_results(*args):
        if len(args) < 1:
            return '[]'

        pattern = args[0]
        glob_pattern = args[1] if len(args) > 1 else None
        time_limit = float(args[2]) if len(args) > 2 else default_time_limit

        if not pattern:
            return '[]'

        base_dir = str(Path(base_directory).resolve())
        result = run_ripgrep(
            pattern=pattern,
            base_directory=base_dir,
            glob_pattern=glob_pattern,
            time_limit=time_limit,
        )

        return json.dumps(result['results'])

    conn.create_function(f'{function_name}_results', -1, ripgrep_results)


if __name__ == '__main__':
    # Demo/test code
    import sqlite3
    import sys

    if len(sys.argv) < 3:
        print("Usage: python sqlite_ripgrep_python.py <directory> <pattern> [glob]")
        sys.exit(1)

    directory = sys.argv[1]
    pattern = sys.argv[2]
    glob = sys.argv[3] if len(sys.argv) > 3 else None

    conn = sqlite3.connect(':memory:')
    register_ripgrep_with_table_helper(conn, directory)

    if glob:
        query = f"SELECT ripgrep(?, ?)"
        result = conn.execute(query, (pattern, glob)).fetchone()[0]
    else:
        query = f"SELECT ripgrep(?)"
        result = conn.execute(query, (pattern,)).fetchone()[0]

    data = json.loads(result)
    print(f"Found {data['count']} results")
    print(f"Truncated: {data['truncated']}")
    print(f"Time limit hit: {data['time_limit_hit']}")

    if data['error']:
        print(f"Error: {data['error']}")

    for r in data['results'][:5]:
        print(f"  {r['path']}:{r['line_number']}: {r['lines'].strip()}")

    if data['count'] > 5:
        print(f"  ... and {data['count'] - 5} more results")

    # Demo table-valued approach
    print("\n--- Table-valued approach using json_each ---")
    query = """
        SELECT
            json_extract(value, '$.path') as path,
            json_extract(value, '$.line_number') as line_number,
            json_extract(value, '$.lines') as line_text
        FROM json_each(ripgrep_results(?))
        LIMIT 5
    """
    for row in conn.execute(query, (pattern,)):
        print(f"  {row[0]}:{row[1]}: {row[2].strip() if row[2] else ''}")
