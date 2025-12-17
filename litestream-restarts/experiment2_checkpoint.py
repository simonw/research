#!/usr/bin/env python3
"""
Litestream S3 Replication Experiment #2: WAL Checkpoint While Stopped

This script tests what happens when the WAL file is checkpointed
(truncated) while litestream is NOT running.

Key finding: Litestream handles this gracefully by creating a new
"generation" (full database snapshot) when it restarts and detects
that the main database file has changed.

Phases:
1. Create SQLite database in WAL mode
2. Start litestream replication to S3
3. Do 1000 commits
4. Stop litestream
5. Do 1000 more commits
6. Force WAL checkpoint (TRUNCATE) - moves WAL data into main db file
7. Restart litestream (creates new generation), do 1000 more commits
8. Restore from S3 backup
9. Compare original and restored databases
"""

import json
import os
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

# Paths
SCRIPT_DIR = Path(__file__).parent.resolve()
LITESTREAM_BIN = SCRIPT_DIR / "litestream"
CREDENTIALS_FILE = SCRIPT_DIR / "credentials.json"
DB_PATH = SCRIPT_DIR / "experiment2.db"
RESTORED_DB_PATH = SCRIPT_DIR / "experiment2_restored.db"
CONFIG_PATH = SCRIPT_DIR / "litestream2.yml"

# S3 bucket
S3_BUCKET = "litestream-experiments"
S3_PATH = f"s3://{S3_BUCKET}/experiment2.db"


def load_credentials():
    """Load AWS credentials from JSON file."""
    with open(CREDENTIALS_FILE) as f:
        return json.load(f)


def create_litestream_config(access_key_id: str, secret_access_key: str):
    """Create litestream configuration file."""
    config = f"""access-key-id: {access_key_id}
secret-access-key: {secret_access_key}

dbs:
  - path: {DB_PATH}
    replicas:
      - url: {S3_PATH}
"""
    with open(CONFIG_PATH, "w") as f:
        f.write(config)
    print(f"Created litestream config at {CONFIG_PATH}")


def create_database():
    """Create SQLite database in WAL mode."""
    # Remove existing database files
    for suffix in ["", "-wal", "-shm"]:
        path = Path(str(DB_PATH) + suffix)
        if path.exists():
            path.unlink()
            print(f"Removed existing {path}")

    # Create new database
    conn = sqlite3.connect(DB_PATH)
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("""
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            phase TEXT NOT NULL,
            sequence INTEGER NOT NULL,
            timestamp REAL NOT NULL,
            message TEXT
        )
    """)
    conn.commit()
    conn.close()

    print(f"Created database at {DB_PATH} in WAL mode")


def start_litestream():
    """Start litestream replication process."""
    print("Starting litestream...")
    process = subprocess.Popen(
        [str(LITESTREAM_BIN), "replicate", "-config", str(CONFIG_PATH)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    # Give it a moment to start up and do initial snapshot
    time.sleep(2)
    if process.poll() is not None:
        stdout, stderr = process.communicate()
        print(f"Litestream failed to start!")
        print(f"stdout: {stdout.decode()}")
        print(f"stderr: {stderr.decode()}")
        sys.exit(1)
    print(f"Litestream started with PID {process.pid}")
    return process


def stop_litestream(process: subprocess.Popen):
    """Stop litestream process gracefully."""
    print(f"Stopping litestream (PID {process.pid})...")
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        print("Litestream didn't stop gracefully, killing...")
        process.kill()
        process.wait()
    print("Litestream stopped")


def show_file_status():
    """Show current database and WAL file status."""
    db_path = Path(DB_PATH)
    wal_path = Path(str(DB_PATH) + "-wal")

    if db_path.exists():
        print(f"  Database file: {db_path.stat().st_size:,} bytes")
    else:
        print(f"  Database file does not exist")

    if wal_path.exists():
        print(f"  WAL file: {wal_path.stat().st_size:,} bytes")
    else:
        print(f"  WAL file does not exist")


def list_generations(access_key_id: str, secret_access_key: str):
    """List litestream generations in S3."""
    env = os.environ.copy()
    env["LITESTREAM_ACCESS_KEY_ID"] = access_key_id
    env["LITESTREAM_SECRET_ACCESS_KEY"] = secret_access_key

    result = subprocess.run(
        [str(LITESTREAM_BIN), "generations", S3_PATH],
        env=env,
        capture_output=True,
        text=True,
    )

    if result.stdout.strip():
        lines = result.stdout.strip().split("\n")
        print(f"  Generations in S3: {len(lines) - 1}")  # -1 for header
        for line in lines:
            print(f"    {line}")
    else:
        print("  No generations in S3")


def do_commits(phase: str, count: int, delay: float = 0.01):
    """
    Perform commits to the database.

    Args:
        phase: Name of the current phase (for logging in DB)
        count: Number of commits to perform
        delay: Delay between commits in seconds
    """
    print(f"Starting {count} commits for phase '{phase}'...")
    conn = sqlite3.connect(DB_PATH)

    for i in range(count):
        conn.execute(
            "INSERT INTO events (phase, sequence, timestamp, message) VALUES (?, ?, ?, ?)",
            (phase, i + 1, time.time(), f"Commit {i + 1} of {count} in phase {phase}")
        )
        conn.commit()
        if delay > 0:
            time.sleep(delay)

        # Progress indicator
        if (i + 1) % 100 == 0:
            print(f"  Phase '{phase}': {i + 1}/{count} commits done")

    conn.close()
    print(f"Completed {count} commits for phase '{phase}'")
    show_file_status()


def force_wal_checkpoint():
    """Force a WAL checkpoint with TRUNCATE mode."""
    print("\n*** FORCING WAL CHECKPOINT (TRUNCATE) ***")
    print("  Before checkpoint:")
    show_file_status()

    conn = sqlite3.connect(DB_PATH)

    # TRUNCATE mode: checkpoint and truncate the WAL file to zero bytes
    cursor = conn.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    result = cursor.fetchone()
    print(f"  Checkpoint result: busy={result[0]}, log={result[1]}, checkpointed={result[2]}")

    conn.close()

    print("  After checkpoint:")
    show_file_status()

    print("*** CHECKPOINT COMPLETE ***\n")


def restore_database(access_key_id: str, secret_access_key: str):
    """Restore database from S3 backup."""
    # Remove existing restored database
    for suffix in ["", "-wal", "-shm"]:
        path = Path(str(RESTORED_DB_PATH) + suffix)
        if path.exists():
            path.unlink()

    print(f"Restoring database from {S3_PATH}...")

    env = os.environ.copy()
    env["LITESTREAM_ACCESS_KEY_ID"] = access_key_id
    env["LITESTREAM_SECRET_ACCESS_KEY"] = secret_access_key

    result = subprocess.run(
        [str(LITESTREAM_BIN), "restore", "-o", str(RESTORED_DB_PATH), S3_PATH],
        env=env,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print(f"Restore failed!")
        print(f"stdout: {result.stdout}")
        print(f"stderr: {result.stderr}")
        sys.exit(1)

    print(f"Database restored to {RESTORED_DB_PATH}")


def compare_databases():
    """Compare original and restored databases."""
    print("\n" + "=" * 60)
    print("DATABASE COMPARISON")
    print("=" * 60)

    # Connect to both databases
    conn_orig = sqlite3.connect(DB_PATH)
    conn_restored = sqlite3.connect(RESTORED_DB_PATH)

    # Count records by phase
    def get_stats(conn, label):
        cursor = conn.execute("SELECT COUNT(*) FROM events")
        total = cursor.fetchone()[0]

        cursor = conn.execute("""
            SELECT phase, COUNT(*), MIN(sequence), MAX(sequence)
            FROM events
            GROUP BY phase
            ORDER BY MIN(id)
        """)
        phases = cursor.fetchall()

        print(f"\n{label}:")
        print(f"  Total records: {total}")
        for phase, count, min_seq, max_seq in phases:
            print(f"  Phase '{phase}': {count} records (seq {min_seq}-{max_seq})")

        return total, phases

    orig_total, orig_phases = get_stats(conn_orig, "Original database")
    restored_total, restored_phases = get_stats(conn_restored, "Restored database")

    # Check for missing records
    print("\n" + "-" * 60)
    print("ANALYSIS")
    print("-" * 60)

    if orig_total == restored_total:
        print(f"SUCCESS: All {orig_total} records were replicated!")
        print("\nThis succeeded because litestream creates a new 'generation'")
        print("(full database snapshot) when it restarts and detects that")
        print("the main database file has changed. The checkpoint moved")
        print("WAL data into the main .db file, and litestream captured it")
        print("in its new snapshot.")
    else:
        missing = orig_total - restored_total
        print(f"DATA LOSS DETECTED!")
        print(f"  Original has {orig_total} records")
        print(f"  Restored has {restored_total} records")
        print(f"  Missing records: {missing}")

        # Find which records are missing
        cursor_orig = conn_orig.execute("SELECT id, phase, sequence FROM events ORDER BY id")
        cursor_restored = conn_restored.execute("SELECT id, phase, sequence FROM events ORDER BY id")

        orig_ids = set(row[0] for row in cursor_orig.fetchall())
        restored_ids = set(row[0] for row in cursor_restored.fetchall())

        missing_ids = orig_ids - restored_ids
        if missing_ids:
            print(f"\n  Sample missing IDs: {sorted(missing_ids)[:20]}{'...' if len(missing_ids) > 20 else ''}")

            # Get details of missing records
            cursor = conn_orig.execute(
                f"SELECT id, phase, sequence FROM events WHERE id IN ({','.join('?' * len(missing_ids))})",
                list(missing_ids)
            )
            missing_records = cursor.fetchall()

            # Group by phase
            by_phase = {}
            for id_, phase, seq in missing_records:
                if phase not in by_phase:
                    by_phase[phase] = []
                by_phase[phase].append(seq)

            print("\n  Missing records by phase:")
            for phase, seqs in sorted(by_phase.items()):
                seqs.sort()
                print(f"    Phase '{phase}': {len(seqs)} records (seq {min(seqs)}-{max(seqs)})")

    conn_orig.close()
    conn_restored.close()


def cleanup_s3(access_key_id: str, secret_access_key: str):
    """Clean up any existing data in S3 before experiment."""
    print(f"Cleaning up existing S3 data at {S3_PATH}...")

    env = os.environ.copy()
    env["LITESTREAM_ACCESS_KEY_ID"] = access_key_id
    env["LITESTREAM_SECRET_ACCESS_KEY"] = secret_access_key

    # List generations to see if anything exists
    result = subprocess.run(
        [str(LITESTREAM_BIN), "generations", S3_PATH],
        env=env,
        capture_output=True,
        text=True,
    )

    if result.stdout.strip():
        print(f"  Found existing generations")
    else:
        print("  No existing data found")


def main():
    print("=" * 60)
    print("LITESTREAM EXPERIMENT #2: WAL CHECKPOINT WHILE STOPPED")
    print("=" * 60)
    print("\nThis experiment tests what happens when the WAL is")
    print("checkpointed while litestream is NOT running.\n")

    # Load credentials
    creds = load_credentials()
    access_key_id = creds["AccessKeyId"]
    secret_access_key = creds["SecretAccessKey"]
    print(f"Loaded credentials for user: {creds['UserName']}")

    # Setup
    cleanup_s3(access_key_id, secret_access_key)
    create_litestream_config(access_key_id, secret_access_key)
    create_database()

    # Phase 1: Start litestream and do 1000 commits
    print("\n" + "-" * 60)
    print("PHASE 1: Litestream running, 1000 commits")
    print("-" * 60)
    litestream_process = start_litestream()
    time.sleep(1)  # Let it initialize
    do_commits("phase1_ls_running", 1000)

    # Give litestream time to replicate
    print("Waiting for litestream to replicate...")
    time.sleep(3)

    print("\nGenerations after Phase 1:")
    list_generations(access_key_id, secret_access_key)

    # Phase 2: Stop litestream and do 1000 commits
    print("\n" + "-" * 60)
    print("PHASE 2: Litestream STOPPED, 1000 commits")
    print("-" * 60)
    stop_litestream(litestream_process)
    time.sleep(1)  # Brief pause
    do_commits("phase2_ls_stopped", 1000)

    # THE KEY DIFFERENCE: Force WAL checkpoint while litestream is stopped
    print("\n" + "-" * 60)
    print("PHASE 2b: CHECKPOINT WAL (while litestream still stopped)")
    print("-" * 60)
    force_wal_checkpoint()

    print("Generations after Phase 2 + checkpoint (still same as before):")
    list_generations(access_key_id, secret_access_key)

    # Phase 3: Restart litestream and do 1000 commits
    print("\n" + "-" * 60)
    print("PHASE 3: Litestream restarted, 1000 commits")
    print("-" * 60)
    litestream_process = start_litestream()
    time.sleep(2)  # Let it initialize and create new generation
    do_commits("phase3_ls_restarted", 1000)

    # Final sync
    print("\nWaiting for final replication...")
    time.sleep(5)

    print("\nGenerations after Phase 3 (should see a NEW generation):")
    list_generations(access_key_id, secret_access_key)

    # Stop litestream
    print("\n" + "-" * 60)
    print("STOPPING LITESTREAM")
    print("-" * 60)
    stop_litestream(litestream_process)

    # Restore and compare
    print("\n" + "-" * 60)
    print("RESTORE AND COMPARE")
    print("-" * 60)
    restore_database(access_key_id, secret_access_key)
    compare_databases()

    print("\n" + "=" * 60)
    print("EXPERIMENT #2 COMPLETE")
    print("=" * 60)


if __name__ == "__main__":
    main()
