#!/usr/bin/env bash
# Reproduce the Litestream 0.5.12 exploration (file + MinIO/S3 backends + MCP).
# Requires: go >= 1.24, docker. Run from this folder.
set -euo pipefail

WORK="$(pwd)/work"
SOCK="$WORK/run/litestream.sock"
LS=/tmp/litestream-bin   # built binary

# ---------- 1. Build litestream v0.5.12 ----------
if [ ! -x "$LS" ]; then
  git clone https://github.com/benbjohnson/litestream /tmp/litestream || true
  ( cd /tmp/litestream && git checkout v0.5.12 &&
    go build -ldflags "-X main.Version=v0.5.12" -o "$LS" ./cmd/litestream )
fi
sudo cp "$LS" /usr/local/bin/litestream   # MCP tools shell out to `litestream`
"$LS" version

mkdir -p "$WORK"/{data,backup,run}

# ---------- 2. Seed a WAL-mode SQLite db ----------
python3 - <<PY
import sqlite3
d=sqlite3.connect("$WORK/data/app.db"); d.execute("PRAGMA journal_mode=WAL")
d.execute("CREATE TABLE IF NOT EXISTS events(id INTEGER PRIMARY KEY, ts TEXT, msg TEXT)")
for i in range(100): d.execute("INSERT INTO events(ts,msg) VALUES(datetime('now'),?)",(f"seed {i}",))
d.commit(); d.close()
PY

# ---------- 3a. FILE backend ----------
setsid "$LS" replicate -config litestream-file.yml >"$WORK/run/daemon-file.log" 2>&1 < /dev/null &
sleep 4
"$LS" sync   -socket "$SOCK" -json "$WORK/data/app.db"
"$LS" list   -socket "$SOCK" -json
"$LS" status -config litestream-file.yml -json
"$LS" ltx    -config litestream-file.yml -level all "$WORK/data/app.db"
"$LS" restore -config litestream-file.yml -o /tmp/restore-file.db -integrity-check full -json "$WORK/data/app.db"
pkill -f "replicate -config litestream-file.yml" || true

# ---------- 3b. MinIO / S3 backend ----------
docker run -d --name minio -p 9000:9000 -p 9001:9001 \
  -e MINIO_ROOT_USER=minioadmin -e MINIO_ROOT_PASSWORD=minioadmin \
  quay.io/minio/minio server /data --console-address ":9001"
sleep 3
docker exec minio mkdir -p /data/litestream          # MinIO bucket = top-level dir

export LITESTREAM_ACCESS_KEY_ID=minioadmin LITESTREAM_SECRET_ACCESS_KEY=minioadmin
setsid "$LS" replicate -config litestream-minio.yml >"$WORK/run/daemon-minio.log" 2>&1 < /dev/null &
sleep 5
S3URL="s3://litestream/app?endpoint=http://localhost:9000&region=us-east-1&force-path-style=true"
"$LS" ltx -level all "$S3URL"
"$LS" restore -o /tmp/from-minio.db -integrity-check full -json "$S3URL"

# ---------- 4. MCP server ----------
# (mcp-addr: localhost:9999 is set in litestream-minio.yml)
python3 mcp_client.py
