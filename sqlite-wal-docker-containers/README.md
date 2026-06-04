# SQLite WAL Mode Across Docker Containers Sharing a Volume

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

## Question

Does SQLite WAL mode work correctly when two Docker containers mount the same volume? WAL mode uses a memory-mapped shared memory file (`.db-shm`) for coordination - if the `mmap` isn't truly shared between containers, reads could see stale data or worse.

## Answer

**Yes, it works correctly on a single Docker host.** Docker containers share the same kernel, so `mmap()` of the same file on a shared volume produces truly shared memory. Both reads and concurrent writes work as expected.

## Experiment

Tested on Docker Desktop for macOS (2026-04-07) using a Docker named volume shared between containers.

### Test 1: Writer + Reader in separate containers

- **Writer container**: Opens DB in WAL mode, inserts 50 rows (100ms apart)
- **Reader container**: Polls the DB every 100ms, reports row count
- **SHM monitor container**: Memory-maps the `.db-shm` file directly, hashes contents each tick

**Results:**
- Reader tracked the writer in real-time, seeing counts increase from 6 to 50
- SHM monitor observed **48 hash changes** across 80 ticks - confirming `mmap` is truly shared
- Zero errors from either container

### Test 2: Two concurrent writers in separate containers

- Both containers write 50 rows each to the same DB simultaneously

**Results:**
- **All 100 rows present** (50 from each writer), zero gaps, zero errors
- SQLite's POSIX file locking worked correctly across containers

## Why It Works

Docker containers on the same host share the same Linux kernel. Named volumes live on the same filesystem. This means:

1. `mmap()` of the same file maps the same page cache pages
2. Writes by one container are instantly visible to another via the shared mapping
3. POSIX `fcntl()` file locks (used by SQLite) work across containers

## When It Would NOT Work

WAL mode breaks when the filesystem doesn't support proper `mmap` or POSIX lock semantics:

- **NFS/CIFS** network mounts across different machines
- **Distributed volume drivers** that sync files across hosts
- **Cloud storage** backends (S3, GCS mounted as FUSE)
- **Any multi-host scenario** where containers run on different machines

The rule: **mmap works if and only if the processes share the same kernel and filesystem.** Same-host Docker containers satisfy this; cross-host setups do not.

## Files

- `writer.py` - Writes 50 rows in WAL mode with 100ms intervals
- `reader.py` - Polls row count, detects stale reads
- `shm_check.py` - Directly mmap's the `.db-shm` file to verify shared memory works
- `run_experiment.sh` - Orchestrates both tests
- `Dockerfile` - Python 3.12 slim image with the scripts
