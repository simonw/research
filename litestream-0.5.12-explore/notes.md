# Litestream 0.5.12 Exploration Notes

Started: 2026-06-15

## Goal
Get Litestream 0.5.12 working, try recent features, with both local file backend and MinIO (local S3 clone).

## Environment
- Go 1.24.7 linux/amd64
- Docker 29.3.1
- Reference clone at /tmp/litestream

## What's new in 0.5.x / 0.5.12 (from reference clone)

Litestream 0.5.x is a major rewrite vs the 0.3.x line most people know:

- **LTX file format** (from `github.com/superfly/ltx`, the LiteFS format) replaces
  the old snapshot + WAL-segment model. Backups are now LTX files organized into
  **compaction levels** (L0 = raw txns, higher levels = compacted).
- **Daemon + control socket architecture**. Instead of one long-lived
  `litestream replicate`, there is now a daemon you `start`/`stop`, and you
  `register`/`unregister`/`sync` databases against it over a Unix control socket
  (default `/var/run/litestream.sock`).
- **New CLI commands**: `start`, `stop`, `register`, `unregister`, `sync`,
  `status`, `list`, `info`, `reset`, `ltx` (replaces deprecated `wal`).
- **Agent/automation-friendly CLI** (the bulk of 0.5.12): `-json` output on
  databases/info/list/ltx/restore/status/sync, dry-run modes for
  restore/reset/unregister, guarded restore overwrites, idempotent
  register/unregister/start/stop (returns `already_<state>`), "Try:" hints.
- **Built-in MCP server** (`mcp.go`, `mcp-addr` config) so AI agents can drive
  Litestream.
- Multi-level compaction config (`levels:`), snapshot/retention config,
  heartbeat URL, validation config, compaction verification.

v0.5.12 tag date: 2026-05-29. Commits since v0.5.11 are almost entirely
`feat(cli)`/`fix(cli)` JSON-output, dry-run, and idempotency work.

### New daemon workflow
1. `litestream start [-config litestream.yml]`  -> starts daemon
2. `litestream register -replica file:///path /db.sqlite`  -> add a db live
3. `litestream sync /db.sqlite`  -> force immediate sync
4. `litestream status|list|info [-json]`  -> query daemon
5. `litestream ltx /db.sqlite [-json]`  -> inspect LTX files/levels
6. `litestream restore -o out.db file:///path` (or from registered db)
7. `litestream stop`

## File backend results (working)

Daemon started via `litestream replicate -config litestream-file.yml`. Config
enabled `socket: {enabled: true, path: ...}`. Multi-level compaction monitors
started automatically: L1 @30s, L2 @5m, L3 @1h, L9(snapshot) @1h, plus an L0
retention monitor @15s (retention 5m).

Backup layout on disk (file replica):
```
backup/app/ltx/0/<minTXID>-<maxTXID>.ltx   # L0 raw per-txn
backup/app/ltx/1/...                         # L1 compacted
backup/app/ltx/9/...                         # L9 snapshot
```

Control commands verified:
- `info -json` / `list -json` use the control SOCKET (`-socket`).
- `databases -json` / `status -json` / `ltx -json` read the CONFIG (`-config`),
  not the socket. (Easy gotcha: status/ltx reject `-socket`.)
- `sync -json` forces an immediate sync and returns `{db_path, txid, duration_ms}`.

### Restore (0.5.x) verified
- `-dry-run` prints the restore plan: source/target/replica + txid range +
  the exact list of LTX files to fetch (starts from an L9 snapshot, then L0 deltas).
- `-json` summary: `{db_path, replica, txid, duration_ms, integrity_check}`.
- `-integrity-check full|quick|none` runs PRAGMA integrity check post-restore.
- **Overwrite guard** (new): refuses if `-o` target exists & non-empty:
  "Use -force to overwrite". `-force` overrides. (was silent-overwrite before)
- **Point-in-time restore**: `-txid 0000000000000002` restored 103 rows vs 110
  at latest tip. `-timestamp` also supported.
- `-f` follow mode: tail-like continuous restore (read replica).

### Live register/start/stop + idempotency (agent features)
- `register -replica file://... -json` adds a DB to the running daemon live;
  re-running returns `status: already_registered`.
- `stop`/`start -json` return `state` + `txid`; re-running yields
  `already_stopped` / `already_running`. No errors on repeat — safe for scripts.
- `unregister -dry-run` previews (incl. "daemon close will sync before completing")
  and sends NO request; db stays listed.
- `reset -dry-run` lists exactly which local LTX files under
  `data/.<db>-litestream/ltx/...` would be removed (local-state dir layout),
  without deleting. Reset forces a fresh snapshot on next sync.
- Errors are agent-friendly: `Error: -replica is required` + a
  `Try: litestream register -replica s3://bucket/prefix /path/to/db` hint, and
  exit code 1. Missing db: `Error: sync failed: database not found: ...`.

Local primary state dir: `data/.<db>-litestream/ltx/<level>/...` (mirrors the
replica's LTX layout).

## MinIO (S3-compatible) backend results

Ran MinIO via `docker run quay.io/minio/minio server /data` (Docker Hub was
rate-limited; quay.io worked). Created the bucket by `mkdir /data/litestream`
inside the container (MinIO treats top-level dirs as buckets).

S3 replica config that worked for MinIO:
```yaml
access-key-id: minioadmin
secret-access-key: minioadmin
dbs:
  - path: .../app.db
    replica:
      type: s3
      bucket: litestream
      path: app
      endpoint: http://localhost:9000
      region: us-east-1
      force-path-style: true     # required for MinIO (path-style, not vhost)
```

Findings:
- Replication to MinIO worked first try; the daemon uploaded all existing local
  LTX state + a snapshot, then incrementally.
- **Credentials gotcha**: S3 *replica URLs* (`s3://...?access-key-id=...`) do NOT
  read creds from query params — it fell back to EC2 IMDS and failed. Supply
  creds via env (`LITESTREAM_ACCESS_KEY_ID` / `LITESTREAM_SECRET_ACCESS_KEY`,
  also AWS_* work) or via the config file. URL query params DO carry
  `endpoint`, `region`, `force-path-style`.
- S3 object key layout: `litestream/app/<zero-padded-level>/<min>-<max>.ltx`
  e.g. `app/0000/...`, `app/0001/...`, `app/0009/...`.
- Multi-level compaction observed end-to-end in MinIO over a few minutes:
  L0=12 files, L1=4, L2=2, L3=1, L9=1 snapshot.
- Full restore from the S3 URL with `-integrity-check full` reproduced 142 rows;
  PITR and dry-run also work against S3.

## Built-in MCP server (new agent feature)

Enabled by adding `mcp-addr: localhost:9999` to the daemon config. It serves a
**Streamable HTTP MCP** endpoint (mark3labs/mcp-go). Exposes 7 tools, all
prefixed `litestream_`: databases, info, ltx, reset, restore, status, version.

How it works internally: each MCP tool **shells out to the `litestream` binary**
on the daemon's PATH (e.g. `litestream version`, `litestream restore ...`) and
returns the text output. So the binary must be named `litestream` and on PATH
(my build was `/tmp/ls`; I had to install it as `/usr/local/bin/litestream`).

Verified over raw HTTP with a hand-written client (`work/mcp_client.py`):
- `initialize` -> serverInfo "Litestream MCP Server 1.0.0", protocol 2024-11-05.
- `tools/list` -> the 7 tools.
- `litestream_version` -> `v0.5.12`.
- `litestream_databases` / `litestream_status` / `litestream_info` -> tables
  (info is a combined report: version + config path + databases + LTX files).
- `litestream_restore {path: <s3 url>, o: /tmp/mcp-restore.db}` -> restored a
  working 142-row DB straight from MinIO, driven entirely over MCP.

MCP tool quirks:
- `litestream_ltx` only accepts `path`/`config` (no `level`), so it lists L0 only.
- `litestream_restore` does NOT expose `-force`/`-dry-run`/`-txid`-guard flags
  that the CLI has (only path/o/config/txid/timestamp/parallelism/if_* ).
- Client gotcha: tool names are server-prefixed `litestream_<name>`.

## Cleanup / env notes
- Multiple orphan daemons can accumulate because the Bash tool's shell job table
  resets between calls; `pkill -f '/tmp/ls replicate'` + `setsid ... & disown`
  to run a clean detached daemon.
