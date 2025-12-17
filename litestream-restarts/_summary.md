Experiments in this project evaluate Litestream’s robustness when SQLite writes occur while Litestream is stopped and later restarted, with focus on replication to S3. Both the simple restart and the scenario where the WAL is checkpointed (truncated) while Litestream is offline confirm no data loss: Litestream either streams pending WAL changes upon restart or detects a database change and uploads a new full snapshot (“generation”). This ensures that S3 replication remains consistent even if Litestream’s process is interrupted, making the tool highly reliable in dynamic environments. Detailed mechanisms and generations can be inspected using [Litestream’s CLI](https://litestream.io/guides/cli/) and the generation listing feature.

Key findings:
- No data loss occurs in either restart or checkpoint scenarios—Litestream recovers via WAL streaming or generation snapshots.
- Creating new generations after checkpointing uploads the entire database, potentially incurring significant storage/bandwidth overhead for large files.
- Frequent restarts with heavy write loads can increase S3 storage due to more generations.
- Litestream’s fallback mechanisms support safe replication during container restarts, server maintenance, or deployment updates.
