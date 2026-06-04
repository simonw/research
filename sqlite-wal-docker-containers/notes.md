# SQLite WAL Mode + Docker Shared Volumes - Research Notes

## Question

If two Docker containers each mount the same volume containing a SQLite database in WAL mode, does WAL mode work correctly? The concern is that the `-shm` (shared memory) file is memory-mapped, and `mmap` may not provide true inter-process shared memory across container boundaries.

## Background

SQLite WAL mode uses three files:
- `.db` - the main database
- `.db-wal` - the write-ahead log
- `.db-shm` - shared memory index into the WAL, used via `mmap()`

The `-shm` file is critical: SQLite uses `mmap()` to map it into each process's address space. Multiple processes on the same host sharing an `mmap()` of the same file get true shared memory - changes by one process are instantly visible to others.

The question: when two Docker containers mount the same volume, do `mmap()` calls on the same file result in truly shared memory pages, or does each container get its own independent mapping?

## Experiment Design

Three scripts:
- **writer.py**: Opens DB in WAL mode, writes 50 rows with 100ms delay between each
- **reader.py**: Polls the DB every 100ms for 80 ticks, tracking how many rows it sees
- **shm_check.py**: Memory-maps the `-shm` file directly and hashes its contents each tick to see if changes from other containers are visible

### Test 1: Writer in container A, reader in container B, shm monitor in container C
All sharing the same Docker named volume.

### Test 2: Two concurrent writers in separate containers
Both write 50 rows each, then verify all 100 rows are present.

## Results

### Test 1: Writer + Reader + SHM Monitor

**Writer**: Successfully wrote all 50 rows, no errors.

**Reader**: Saw counts progressively increase from 6 to 50. Every write was visible. The "max stale streak of 35" was just the reader continuing to poll after the writer finished (seeing 50 repeatedly). The reader saw 45 distinct count values (6 through 50) - it was tracking the writer in near-real-time.

**SHM Monitor**: Hash of the mmap'd shm file changed 48 times across 80 ticks. This proves that **mmap IS truly shared** across containers when they mount the same Docker volume.

### Test 2: Two Concurrent Writers

- WRITER-A: 50 rows written, no errors
- WRITER-B: 50 rows written, no errors
- **All 100 rows present**, values 1-50 for each writer, no gaps, no corruption
- SQLite's locking worked correctly across container boundaries

## Analysis

**WAL mode works correctly with Docker named volumes on a single host.** Here's why:

Docker containers on the same host share the same Linux kernel. Docker named volumes are stored on the same filesystem within the Docker VM (on macOS) or directly on the host filesystem (on Linux). When two containers mount the same named volume:

1. They access the same physical files via the same filesystem
2. `mmap()` calls on the same file produce mappings backed by the same page cache pages
3. Changes by one container's process are immediately visible to the other
4. POSIX file locks (used by SQLite for write coordination) work correctly

This is fundamentally different from **network filesystems** (NFS, CIFS, cloud storage) where mmap semantics break down because there's no shared page cache.

## Important Caveats

The experiment was run on Docker Desktop for macOS, where containers run inside a single Linux VM. The results should be identical on:
- Native Docker on Linux (same kernel, same filesystem)
- Docker Desktop on Windows (also uses a Linux VM)

WAL mode would **NOT** work correctly with:
- NFS-mounted volumes shared across different hosts
- Distributed/network filesystems
- Volumes shared across different Docker hosts (e.g., via a volume driver that syncs files)
- Any scenario where the two containers are on different machines

The key principle: **mmap works if and only if the processes share the same kernel and filesystem.** Docker containers on the same host satisfy this requirement.
