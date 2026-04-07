#!/bin/bash
set -e

DIR="$(cd "$(dirname "$0")" && pwd)"
VOLUME_NAME="sqlite-wal-test-vol"
IMAGE_NAME="sqlite-wal-test"

echo "=== Building Docker image ==="
docker build -t "$IMAGE_NAME" "$DIR"

echo ""
echo "=== Cleaning up any previous run ==="
docker rm -f wal-writer wal-reader wal-shm-monitor 2>/dev/null || true
docker volume rm "$VOLUME_NAME" 2>/dev/null || true
docker volume create "$VOLUME_NAME"

echo ""
echo "=========================================="
echo "  TEST 1: Writer + Reader in separate containers"
echo "=========================================="
echo ""

# Start writer
docker run -d --name wal-writer \
    -v "$VOLUME_NAME":/data \
    -e CONTAINER_ID=WRITER \
    "$IMAGE_NAME" python /app/writer.py

# Start reader in separate container
docker run -d --name wal-reader \
    -v "$VOLUME_NAME":/data \
    -e CONTAINER_ID=READER \
    "$IMAGE_NAME" python /app/reader.py

# Start shm monitor in a third container
docker run -d --name wal-shm-monitor \
    -v "$VOLUME_NAME":/data \
    -e CONTAINER_ID=SHM-MON \
    "$IMAGE_NAME" python /app/shm_check.py

echo "Waiting for containers to finish..."
docker wait wal-writer wal-reader wal-shm-monitor 2>/dev/null || true

echo ""
echo "--- WRITER OUTPUT ---"
docker logs wal-writer
echo ""
echo "--- READER OUTPUT ---"
docker logs wal-reader
echo ""
echo "--- SHM MONITOR OUTPUT ---"
docker logs wal-shm-monitor

# Clean up test 1
docker rm -f wal-writer wal-reader wal-shm-monitor 2>/dev/null || true
docker volume rm "$VOLUME_NAME" 2>/dev/null || true
docker volume create "$VOLUME_NAME"

echo ""
echo "=========================================="
echo "  TEST 2: Two concurrent writers in separate containers"
echo "=========================================="
echo ""

docker run -d --name wal-writer-a \
    -v "$VOLUME_NAME":/data \
    -e CONTAINER_ID=WRITER-A \
    "$IMAGE_NAME" python /app/writer.py

sleep 0.5

docker run -d --name wal-writer-b \
    -v "$VOLUME_NAME":/data \
    -e CONTAINER_ID=WRITER-B \
    "$IMAGE_NAME" python /app/writer.py

echo "Waiting for both writers..."
docker wait wal-writer-a wal-writer-b 2>/dev/null || true

echo ""
echo "--- WRITER A OUTPUT ---"
docker logs wal-writer-a

echo ""
echo "--- WRITER B OUTPUT ---"
docker logs wal-writer-b

# Now verify data integrity
echo ""
echo "--- DATA INTEGRITY CHECK ---"
docker run --rm \
    -v "$VOLUME_NAME":/data \
    "$IMAGE_NAME" python -c "
import sqlite3
conn = sqlite3.connect('/data/test.db')
conn.execute('PRAGMA journal_mode=WAL')
total = conn.execute('SELECT COUNT(*) FROM events').fetchone()[0]
by_container = conn.execute('SELECT container, COUNT(*) FROM events GROUP BY container').fetchall()
print(f'Total rows: {total}')
for c, n in by_container:
    print(f'  {c}: {n} rows')
if total == 100:
    print('PASS: All 100 rows present (50 from each writer)')
else:
    print(f'FAIL: Expected 100 rows, got {total}')

# Check for duplicates or gaps
for container_id in ['WRITER-A', 'WRITER-B']:
    vals = [r[0] for r in conn.execute('SELECT value FROM events WHERE container=? ORDER BY value', (container_id,)).fetchall()]
    expected = list(range(1, 51))
    if vals == expected:
        print(f'  {container_id}: values 1-50 all present, no gaps')
    else:
        print(f'  {container_id}: MISMATCH - got {len(vals)} values, expected 50')
        missing = set(expected) - set(vals)
        if missing:
            print(f'    Missing values: {sorted(missing)}')
conn.close()
"

echo ""
echo "=== Cleanup ==="
docker rm -f wal-writer-a wal-writer-b 2>/dev/null || true
docker volume rm "$VOLUME_NAME" 2>/dev/null || true

echo ""
echo "=== EXPERIMENT COMPLETE ==="
