# Exploring Litestream 0.5.12

[Litestream](https://github.com/benbjohnson/litestream) is a disaster-recovery
tool that replicates a live SQLite database to another file or to S3. I built
the **v0.5.12** release (tagged 2026-05-29) from source and exercised its newer
features against two backends: a **local file** replica and **MinIO** (a local,
S3-compatible object store) run in Docker.

The headline finding: the 0.5.x line is a *major* departure from the 0.3.x
Litestream most people know. It replaces the old "snapshot + WAL-segment" format
with the **LTX file format** and a **multi-level compaction** scheme, adds a
long-running **daemon with a control socket**, and — the focus of 0.5.12 — a
large pile of **agent/automation-friendly** CLI features (JSON output, dry-run
modes, idempotent commands, structured errors) plus a **built-in MCP server**.

## TL;DR — what's new and what I verified

| Area | New in 0.5.x / 0.5.12 | Verified here |
| --- | --- | --- |
| Storage format | LTX files in compaction **levels** (L0 raw → L9 snapshot) | ✅ saw L0/L1/L2/L3/L9 on disk and in MinIO |
| Daemon model | `replicate` daemon + Unix **control socket**; live `register`/`unregister`/`start`/`stop`/`sync` | ✅ added/removed DBs against a running daemon |
| JSON output | `-json` on databases/info/list/ltx/status/sync/restore | ✅ all return the documented schemas |
| Restore | `-dry-run`, `-force` overwrite **guard**, `-integrity-check`, PITR (`-txid`/`-timestamp`), `-f` follow mode | ✅ each tested |
| Idempotency | repeat commands return `already_<state>` instead of erroring | ✅ `already_registered/running/stopped` |
| Agent errors | `Error: ...` + a `Try: ...` hint, exit code 1 | ✅ |
| MCP server | built-in Streamable-HTTP MCP server (`mcp-addr`) exposing 7 tools | ✅ drove a full restore over MCP |
| S3 backends | path-style + custom `endpoint` for S3 clones | ✅ MinIO worked first try |

## The new architecture in one picture

```
                 ┌──────────────────────────────────────────┐
  CLI commands   │  litestream replicate  (the daemon)        │
  register ───▶  │   ├─ control socket  /var/run/litestream.sock
  start/stop ─▶  │   ├─ compaction monitors  L1@30s L2@5m L3/L9@1h
  sync ──────▶  │   ├─ L0 retention monitor @15s             │
  status/ltx     │   └─ MCP server (if mcp-addr set)          │
  (read config)  └───────────────┬────────────────────────────┘
                                  ▼
        LTX files by level →  file:///backup/app/ltx/<level>/<min>-<max>.ltx
                              s3://bucket/app/<zero-padded-level>/<min>-<max>.ltx
```

* **`replicate`** is the daemon (was the only real command in 0.3.x).
* **`start`/`stop`** toggle replication for an *already-registered* DB on the
  running daemon; **`register`/`unregister`** add/remove DBs live over the socket.
* **`sync`** forces an immediate replica sync.
* **`status`/`ltx`/`databases`** read the **config file** (`-config`); **`info`/
  `list`/`sync`/`register`/`start`/`stop`** talk to the **socket** (`-socket`).
  (Mixing these up is the most common gotcha — `status` rejects `-socket`.)

## Backend 1 — local file replica

`litestream-file.yml` enables the control socket and a `type: file` replica.
After starting `litestream replicate -config litestream-file.yml`, the daemon
immediately began emitting LTX files:

```
backup/app/ltx/0/0000000000000001-0000000000000001.ltx   # L0: one file per txn
backup/app/ltx/1/0000000000000001-0000000000000009.ltx   # L1: compacted range
backup/app/ltx/9/0000000000000001-0000000000000009.ltx   # L9: full snapshot
```

Things I confirmed:

* `sync -json` → `{"db_path":..., "txid":4, "duration_ms":8}`.
* `ltx -level all -json` lists every level with `min_txid`/`max_txid`/`size`.
* **Restore dry-run** prints the exact plan — which L9 snapshot + which L0 deltas
  it would fetch — without writing anything.
* **Restore overwrite guard** (new): restoring onto an existing path now refuses
  with *"output path already exists and is not empty … Use -force to
  overwrite"*. `-force` overrides. (0.3.x silently clobbered.)
* **Point-in-time restore**: `-txid 0000000000000002` rebuilt the DB at 103 rows
  vs 110 at the tip.
* **Follow mode** (`restore -f`): a read replica that tails the primary. Wrote 7
  rows to the primary, forced a sync, and the follower advanced
  `from_txid=7 to_txid=8` and reached 122 rows on its own.
* `-integrity-check full` runs `PRAGMA integrity_check` after restore.

## Backend 2 — MinIO (local S3 clone)

MinIO was run with Docker (`quay.io/minio/minio` — Docker Hub was rate-limited),
and a bucket was created simply by `mkdir`-ing a directory under its `/data`.
The working S3 replica config (`litestream-minio.yml`):

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
      force-path-style: true     # required for MinIO (path-style addressing)
```

Replication worked on the first try. Over a few minutes the multi-level
compaction filled out in the bucket:

```
app/0000/…  (L0, 12 files)   app/0002/…  (L2, 2)   app/0009/…  (L9 snapshot, 1)
app/0001/…  (L1, 4 files)    app/0003/…  (L3, 1)
```

A full `restore -integrity-check full` straight from the S3 URL reproduced all
142 rows and passed the integrity check.

**Credentials gotcha worth knowing:** S3 *replica URLs*
(`s3://bucket/app?...`) do **not** pick up credentials from query parameters —
Litestream falls back to EC2 IMDS and fails. Supply credentials via environment
(`LITESTREAM_ACCESS_KEY_ID` / `LITESTREAM_SECRET_ACCESS_KEY`, or `AWS_*`) or the
config file. The URL *does* carry `endpoint`, `region`, and `force-path-style`.

## The built-in MCP server (the standout new feature)

Setting `mcp-addr: localhost:9999` in the daemon config starts a **Streamable
HTTP MCP server** (built on `mark3labs/mcp-go`) exposing 7 tools, all prefixed
`litestream_`: `databases`, `info`, `ltx`, `reset`, `restore`, `status`,
`version`.

Internally each tool simply **shells out to the `litestream` binary** on the
daemon's `PATH` and returns its text output — so the binary must be installed as
`litestream` (a non-obvious requirement when you've built it under another name).

I drove it end-to-end with a hand-written HTTP client (`mcp_client.py`):

* `initialize` → `Litestream MCP Server 1.0.0`, protocol `2024-11-05`.
* `tools/list` → the 7 tools.
* `litestream_info` returns a combined report (version + config + databases + LTX
  files).
* **`litestream_restore`** with `{path: <MinIO S3 URL>, o: /tmp/mcp-restore.db}`
  restored a working 142-row database **entirely over MCP**.

Caveats: the MCP `ltx` tool only takes `path`/`config` (no `level`, so it lists
L0 only), and `restore` exposes fewer flags than the CLI (no `-force`/`-dry-run`).
Sample transcript: [`mcp-demo-output.txt`](mcp-demo-output.txt).

## Files in this folder

| File | What it is |
| --- | --- |
| `notes.md` | Running lab notes captured while exploring |
| `reproduce.sh` | End-to-end repro: build → file backend → MinIO → MCP |
| `litestream-file.yml` | Daemon config for the local file replica |
| `litestream-minio.yml` | Daemon config for the MinIO/S3 replica + MCP server |
| `mcp_client.py` | Minimal MCP Streamable-HTTP client used to drive the server |
| `mcp-demo-output.txt` | Captured output of the MCP session |

> Configs use absolute paths under this folder's `work/` directory; adjust them
> if you relocate the experiment. Requires Go ≥ 1.24 and Docker.

## Takeaways

* Litestream 0.5.x is genuinely a new tool: LTX + leveled compaction + a daemon
  you talk to over a socket. The mental model from 0.3.x ("one `replicate`
  process per DB") no longer fully applies.
* 0.5.12 specifically is an **"AI-agent ergonomics" release** — almost every
  commit since 0.5.11 is JSON output, dry-run, idempotency, hint-rich errors, or
  the MCP server. It is clearly designed to be driven by automation/agents, not
  just humans.
* Both backends "just work". MinIO needs only `force-path-style: true` and a
  custom `endpoint`; the main friction is remembering that replica-URL
  credentials come from the environment, not the URL.
