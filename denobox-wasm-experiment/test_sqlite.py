#!/usr/bin/env python3
"""
Test SQLite (sql.js) via Denobox.
"""

from denobox import Denobox, DenoboxError
import base64

print("=== Testing SQLite (sql.js) via Denobox ===\n")

# Read the files
wasm_path = "/tmp/sql-wasm.wasm"
js_path = "/tmp/sql-wasm.js"

with open(wasm_path, 'rb') as f:
    wasm_bytes = f.read()
    wasm_b64 = base64.b64encode(wasm_bytes).decode('ascii')

with open(js_path, 'r') as f:
    js_glue = f.read()

print(f"WASM size: {len(wasm_bytes)} bytes ({len(wasm_bytes)/1024:.1f} KB)")
print(f"JS size: {len(js_glue)} bytes ({len(js_glue)/1024:.1f} KB)")
print()

# JavaScript code to run SQLite
js_code = '''
(async () => {
    // Decode WASM bytes
    const wasmB64 = "''' + wasm_b64 + '''";
    const wasmBytes = Uint8Array.from(atob(wasmB64), c => c.charCodeAt(0));

    // Create environment for sql.js
    const getInitSqlJs = function() {
        var process = { versions: { node: false } };
        var exports = {};
        var module = { exports: exports };

        ''' + js_glue + '''

        return module.exports;
    };

    const initSqlJs = getInitSqlJs();

    // Initialize SQL.js with the WASM binary
    const SQL = await initSqlJs({
        wasmBinary: wasmBytes.buffer
    });

    // Create an in-memory database
    const db = new SQL.Database();

    // Create a table
    db.run("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)");

    // Insert some data
    db.run("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com')");
    db.run("INSERT INTO users VALUES (2, 'Bob', 'bob@example.com')");
    db.run("INSERT INTO users VALUES (3, 'Charlie', 'charlie@example.com')");

    // Query the data
    const results = db.exec("SELECT * FROM users WHERE id > 1");

    // Get some metadata
    const countResult = db.exec("SELECT COUNT(*) as count FROM users");

    // Close the database
    db.close();

    return {
        success: true,
        query_results: results,
        count: countResult[0].values[0][0]
    };
})()
'''

print("Executing SQLite in Denobox...")

with Denobox() as box:
    try:
        result = box.eval(js_code)
        print(f"\nResult: {result}")
        if result.get('success'):
            print(f"\nSQLite Query Results:")
            for table in result.get('query_results', []):
                print(f"  Columns: {table['columns']}")
                for row in table['values']:
                    print(f"    Row: {row}")
            print(f"\nTotal users in table: {result.get('count')}")
            print("\nSuccess! SQLite is running inside Denobox!")
    except DenoboxError as e:
        print(f"DenoboxError: {e}")
    except Exception as e:
        print(f"Error: {type(e).__name__}: {e}")

print("\n=== Test completed ===")
