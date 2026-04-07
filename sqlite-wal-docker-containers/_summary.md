SQLite’s WAL mode reliably supports concurrent access when two Docker containers share a volume on the same host, due to shared kernel and filesystem semantics. The experiment, using [Docker Desktop](https://www.docker.com/products/docker-desktop/) for macOS and a named volume, demonstrated real-time propagation of database changes and effective memory-mapped file sharing by monitoring `.db-shm`. Both reading and concurrent writing tests returned zero errors, with all expected data visible in real time, confirming that mmap and POSIX file locking function as intended across containers. However, these guarantees fail in distributed or multi-host scenarios, or with network filesystems that lack proper mmap and locking support.

**Key findings:**
- WAL mode works reliably for SQLite across containers on the same host.
- Memory mapping (`mmap()`) and file locking are genuinely shared in Docker’s named volumes.
- Issues arise with NFS, CIFS, or distributed/cloud filesystems, where mmap and locks cannot synchronize across machines.
- See scripts and orchestration details in [the experiment toolkit](https://github.com/yourproject/sqlite-wal-docker-example).
