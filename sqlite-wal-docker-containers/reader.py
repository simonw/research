"""Reader process: polls a SQLite WAL-mode database and reports what it sees."""
import sqlite3
import time
import os
import sys

DB_PATH = "/data/test.db"
CONTAINER_ID = os.environ.get("CONTAINER_ID", "unknown")

def main():
    # Wait for the DB to exist
    for _ in range(30):
        if os.path.exists(DB_PATH):
            break
        time.sleep(0.2)
    else:
        print(f"[{CONTAINER_ID}] DB never appeared!", flush=True)
        sys.exit(1)

    time.sleep(0.5)  # Let writer create table

    conn = sqlite3.connect(DB_PATH, timeout=10)
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("PRAGMA busy_timeout=5000")

    print(f"[{CONTAINER_ID}] Reader started", flush=True)

    last_count = 0
    stale_streak = 0
    max_stale_streak = 0
    readings = []

    for tick in range(80):
        try:
            rows = conn.execute(
                "SELECT COUNT(*) FROM events"
            ).fetchone()
            count = rows[0]

            if count == last_count and count > 0:
                stale_streak += 1
                max_stale_streak = max(max_stale_streak, stale_streak)
            else:
                stale_streak = 0

            if count != last_count:
                print(f"[{CONTAINER_ID}] Read count={count} (tick {tick})", flush=True)
                last_count = count

            readings.append(count)
        except sqlite3.OperationalError as e:
            print(f"[{CONTAINER_ID}] READ ERROR: {e}", flush=True)
            readings.append(-1)

        time.sleep(0.1)

    # Summary
    print(f"\n[{CONTAINER_ID}] === READER SUMMARY ===", flush=True)
    print(f"[{CONTAINER_ID}] Final count seen: {last_count}", flush=True)
    print(f"[{CONTAINER_ID}] Max consecutive stale reads: {max_stale_streak}", flush=True)
    print(f"[{CONTAINER_ID}] Total readings: {len(readings)}", flush=True)

    # Check distinct values seen
    distinct = sorted(set(readings))
    print(f"[{CONTAINER_ID}] Distinct counts observed: {distinct}", flush=True)

    if max_stale_streak > 20:
        print(f"[{CONTAINER_ID}] WARNING: Very long stale streak suggests reader is NOT seeing writer's commits!", flush=True)

    conn.close()

if __name__ == "__main__":
    main()
