#!/usr/bin/env python3
"""
Litestream S3 Replication Experiment

This script tests what happens when litestream is stopped and restarted
while writes are ongoing to a SQLite database.

Phases:
1. Create SQLite database in WAL mode
2. Start litestream replication to S3
3. Do 1000 commits
4. Stop litestream, do 1000 more commits
5. Restart litestream, do 1000 more commits
6. Stop litestream
7. Restore from S3 backup
8. Compare original and restored databases
"""

import json
import os
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# Paths
SCRIPT_DIR = Path(__file__).parent.resolve()
LITESTREAM_BIN = SCRIPT_DIR / "litestream"
CREDENTIALS_FILE = SCRIPT_DIR / "credentials.json"
DB_PATH = SCRIPT_DIR / "experiment.db"
RESTORED_DB_PATH = SCRIPT_DIR / "experiment_restored.db"
CONFIG_PATH = SCRIPT_DIR / "litestream.yml"

# S3 bucket
S3_BUCKET = "litestream-experiments"
S3_PATH = f"s3://{S3_BUCKET}/experiment.db"


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


def restore_database(access_key_id: str, secret_access_key: str):
    """Restore database from S3 backup."""
    # Remove existing restored database
    if RESTORED_DB_PATH.exists():
        RESTORED_DB_PATH.unlink()

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
    else:
        missing = orig_total - restored_total
        print(f"DIFFERENCE: Original has {orig_total}, restored has {restored_total}")
        print(f"  Missing records: {missing}")

        # Find which records are missing
        cursor_orig = conn_orig.execute("SELECT id, phase, sequence FROM events ORDER BY id")
        cursor_restored = conn_restored.execute("SELECT id, phase, sequence FROM events ORDER BY id")

        orig_ids = set(row[0] for row in cursor_orig.fetchall())
        restored_ids = set(row[0] for row in cursor_restored.fetchall())

        missing_ids = orig_ids - restored_ids
        if missing_ids:
            print(f"\n  Missing record IDs: {sorted(missing_ids)[:20]}{'...' if len(missing_ids) > 20 else ''}")

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
            for phase, seqs in by_phase.items():
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
    print("LITESTREAM S3 REPLICATION EXPERIMENT")
    print("=" * 60)

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

    # Phase 2: Stop litestream and do 1000 commits
    print("\n" + "-" * 60)
    print("PHASE 2: Litestream STOPPED, 1000 commits")
    print("-" * 60)
    stop_litestream(litestream_process)
    time.sleep(1)  # Brief pause
    do_commits("phase2_ls_stopped", 1000)

    # Phase 3: Restart litestream and do 1000 commits
    print("\n" + "-" * 60)
    print("PHASE 3: Litestream restarted, 1000 commits")
    print("-" * 60)
    litestream_process = start_litestream()
    time.sleep(1)  # Let it initialize and catch up
    do_commits("phase3_ls_restarted", 1000)

    # Final sync
    print("\nWaiting for final replication...")
    time.sleep(5)

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
    print("EXPERIMENT COMPLETE")
    print("=" * 60)


if __name__ == "__main__":
    main()
