# SeaweedFS Feature Testing Notes

## Setup
- Cloned https://github.com/seaweedfs/seaweedfs to /tmp/seaweedfs
- Downloaded binary from release 4.12 for linux_amd64
- Binary: `/tmp/weed` - version 30GB 4.12 0b80f055c285481eba4ca62ebee7341872c81092
- Used `weed mini -dir=/tmp/seaweedfs-data -ip=127.0.0.1 -ip.bind=127.0.0.1` for all-in-one mode
- Used `uvx showboat` to create executable demo document (README.md)
- Used `uvx rodney` for browser automation/screenshots of web UIs

## Tools
- **showboat**: Creates markdown documents with executable code blocks and captured output
- **rodney**: Chrome automation CLI for navigation, screenshots, and page interaction

## What Worked

### `weed mini` All-in-One Mode
- Single command starts master (9333), volume server (9340), filer (8888), S3 (8333), WebDAV (7333), admin UI (23646)
- Auto-configures volume size (512MB) based on disk
- Auto-creates 7 initial volumes
- Maintenance manager runs background tasks (TTL cleanup, etc.)

### Blob Store API
- Two-step process: `GET /dir/assign` then `POST file to URL/fid`
- Volume lookup via `GET /dir/lookup?volumeId=N`
- Multiple URL formats work: `/vol,fid`, `/vol/fid/name.ext`, `/vol/fid.ext`
- DELETE returns 404 on subsequent reads
- ETag, Last-Modified, Accept-Ranges headers all present

### Filer
- Upload via multipart POST to directory paths
- Auto-creates directory structure
- JSON listing via `Accept: application/json` header
- Full POSIX-like metadata (mode, uid, gid, mtime, crtime)
- Each file maps to blob store chunks

### S3 API
- Created credentials via filer's `.configurator/s3.identities.json`
- AWS CLI works without modification (just `--endpoint-url`)
- Bucket create, file upload/download, listing, deletion all work
- Subdirectory structures within buckets work

### WebDAV
- PUT uploads, MKCOL creates directories
- PROPFIND lists contents (XML response)
- GET downloads, DELETE removes
- Supports locking (lockentry in PROPFIND response)

### TTL
- Blob store: `?ttl=1m` on assign creates time-limited volumes
- TTL volumes auto-expire: volume 18 was completely deleted after ~6 minutes
- Maintenance manager handles cleanup

### Collections
- `?collection=name` on assign routes to dedicated volumes
- Each collection gets 7 volumes (matching the default volume count)
- S3 buckets automatically create their own collection
- Useful for namespace isolation and bulk deletion

### Compression
- Transparent to client (Content-Length always shows original size)
- Internal compression at storage layer
- Text files compressed, binary/random data stored as-is
- MD5 checksums match after roundtrip

### Image Resizing
- `?width=N&height=N` parameters on blob URLs
- `mode=fit` (maintain aspect ratio) and `mode=fill` (may crop) supported
- Dramatic size reduction: 123KB original -> 557B at 200x200

### Volume Management
- `/vol/vacuum` triggers cluster-wide compaction
- `garbageThreshold` parameter controls minimum garbage ratio
- CompactRevision increments after successful vacuum
- Dead space from deleted files is reclaimed

### Replication
- `?replication=000` works on single node
- Higher replication levels correctly reject when insufficient nodes
- `?dataCenter=name` parameter routes to specific data center

## Issues Encountered

1. **gRPC connection failures on first start**: When using the default IP (non-localhost), gRPC connections failed repeatedly. Fixed by using `-ip=127.0.0.1 -ip.bind=127.0.0.1`

2. **Admin UI SPA navigation**: The admin UI uses hash-based routing (#/buckets etc.) but `rodney open` with hash URLs caused Chrome errors. Had to stay on dashboard for screenshots.

3. **Volume server /status endpoint temporarily unavailable**: After running vacuum, the `/status` endpoint returned empty responses for a few seconds. The server was still running - just temporarily busy with compaction.

4. **Python f-string escaping in showboat**: Embedding Python with f-strings and dictionary access inside bash inside showboat caused escaping nightmares. Solved by writing Python to files first.

## Features Not Testable in Single-Node Setup
- Multi-node replication (001, 010, 100, 200, 110)
- Erasure coding
- FUSE mount (requires fuse kernel module)
- Cloud tiering (requires cloud credentials)
- Active-active cross-cluster replication
- Hadoop integration
