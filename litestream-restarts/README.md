# Litestream S3 Replication Experiments

This repository contains experiments testing Litestream's behavior when replicating SQLite databases to S3, specifically focusing on what happens when Litestream is stopped and restarted while writes continue.

## Background

[Litestream](https://litestream.io/) replicates SQLite databases by streaming Write-Ahead Log (WAL) changes to a replica destination (S3 in our case). The key question: **What happens to data written while Litestream is not running?**

## Setup

- `litestream` - Litestream binary (v0.3.x)
- `credentials.json` - AWS credentials for S3 access
- S3 bucket: `litestream-experiments`

## Experiments

### Experiment 1: Basic Restart (`experiment.py`)

**Scenario:** Stop Litestream, write data, restart Litestream.

**Phases:**
1. Start Litestream, write 1000 commits
2. Stop Litestream, write 1000 commits (WAL accumulates)
3. Restart Litestream, write 1000 commits
4. Restore from S3 and compare

**Result:** All 3000 records replicated successfully.

```
Original database:
  Total records: 3000
  Phase 'phase1_ls_running': 1000 records
  Phase 'phase2_ls_stopped': 1000 records
  Phase 'phase3_ls_restarted': 1000 records

Restored database:
  Total records: 3000
  (identical to original)
```

**Why it works:** When Litestream restarts, it reads from where it left off in the WAL file. As long as the WAL hasn't been checkpointed (truncated), all changes are preserved and will be replicated.

---

### Experiment 2: WAL Checkpoint While Stopped (`experiment2_checkpoint.py`)

**Scenario:** Stop Litestream, write data, checkpoint the WAL (moving data from WAL into main database file), then restart Litestream.

**Phases:**
1. Start Litestream, write 1000 commits
2. Stop Litestream, write 1000 commits
3. Force WAL checkpoint (TRUNCATE) - moves WAL data into main .db file
4. Restart Litestream, write 1000 commits
5. Restore from S3 and compare

**Result:** All 3000 records replicated successfully.

```
Generations after Phase 1:  5
Generations after Phase 3:  6  <-- NEW generation created!

Original database:  3000 records
Restored database:  3000 records (identical)
```

**Why it works:** Litestream tracks the database file's state. When it restarts and detects the main database file has changed (different size/checksum than expected), it creates a **new generation** - a fresh full snapshot of the database. This captures all data that was checkpointed into the main file.

## Key Findings

### Litestream's Resilience Mechanisms

| Scenario | Mechanism | Data Loss? |
|----------|-----------|------------|
| WAL preserved during restart | Continues reading WAL from last position | No |
| WAL checkpointed during restart | Creates new generation (full snapshot) | No |

### What is a "Generation"?

A generation in Litestream represents a continuous stream of replication starting from a base snapshot. Litestream creates a new generation when:

- Initial replication starts
- The database file changes unexpectedly (e.g., after a checkpoint while Litestream was stopped)
- The WAL sequence becomes inconsistent

You can view generations with:
```bash
litestream generations s3://bucket/database.db
```

### SQLite WAL Auto-Checkpoint Behavior

SQLite automatically checkpoints the WAL when:
- All database connections close
- The WAL reaches a size threshold (default: 1000 pages)
- An explicit `PRAGMA wal_checkpoint` is issued

In our experiments, we observed that SQLite often auto-checkpointed when connections closed, which is why the WAL file frequently didn't exist when we tried to manually checkpoint.

## Implications for Production

1. **Litestream restarts are safe** - Whether the WAL is preserved or checkpointed, Litestream will recover and replicate all data.

2. **New generations have overhead** - Creating a new generation requires uploading a full database snapshot. For large databases, this can be significant.

3. **Minimize restarts during heavy writes** - While data won't be lost, frequent restarts during heavy write loads could create many generations and increase S3 storage/bandwidth.

4. **Restore uses latest generation** - By default, `litestream restore` uses the most recent generation, which will have the most complete data.

## Running the Experiments

```bash
# Experiment 1: Basic restart
uv run python experiment.py

# Experiment 2: WAL checkpoint scenario
uv run python experiment2_checkpoint.py
```

## Files Created

After running experiments:
- `experiment.db` / `experiment2.db` - Original SQLite databases
- `experiment_restored.db` / `experiment2_restored.db` - Restored from S3
- `litestream.yml` / `litestream2.yml` - Litestream configuration files

## Conclusion

Litestream is remarkably resilient to restart scenarios. The generation system ensures that even if WAL continuity is broken (by checkpointing while Litestream is stopped), no data is lost because Litestream will create a fresh snapshot of the database when it detects changes.

This makes Litestream suitable for environments where the replication process might be interrupted, such as:
- Container restarts
- Server maintenance
- Process crashes
- Deployment updates

The key insight is that Litestream doesn't solely rely on WAL streaming - it has fallback mechanisms to ensure data integrity through full database snapshots when needed.
