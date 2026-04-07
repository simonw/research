"""Writer process: inserts rows into a SQLite WAL-mode database."""
import sqlite3
import time
import os
import sys

DB_PATH = "/data/test.db"
CONTAINER_ID = os.environ.get("CONTAINER_ID", "unknown")

def main():
    conn = sqlite3.connect(DB_PATH, timeout=10)
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("PRAGMA busy_timeout=5000")
    conn.execute("""
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            container TEXT,
            value INTEGER,
            ts REAL
        )
    """)
    conn.commit()

    print(f"[{CONTAINER_ID}] Writer started, WAL mode enabled", flush=True)

    for i in range(1, 51):
        try:
            conn.execute(
                "INSERT INTO events (container, value, ts) VALUES (?, ?, ?)",
                (CONTAINER_ID, i, time.time())
            )
            conn.commit()
            print(f"[{CONTAINER_ID}] Wrote value={i}", flush=True)
        except sqlite3.OperationalError as e:
            print(f"[{CONTAINER_ID}] WRITE ERROR at value={i}: {e}", flush=True)
        time.sleep(0.1)

    # Final count
    row = conn.execute("SELECT COUNT(*) FROM events WHERE container=?", (CONTAINER_ID,)).fetchone()
    print(f"[{CONTAINER_ID}] Writer done. Rows written by me: {row[0]}", flush=True)
    conn.close()

if __name__ == "__main__":
    main()
