"""Directly inspect the -shm file to check if mmap is truly shared across containers.

This script memory-maps the .db-shm file and polls it for changes,
printing a hash of its contents each time. If the shm is truly shared,
we should see the hash change as the writer in another container modifies it.
If not shared, the hash will remain static.
"""
import mmap
import hashlib
import time
import os
import sys

DB_PATH = "/data/test.db"
SHM_PATH = DB_PATH + "-shm"
CONTAINER_ID = os.environ.get("CONTAINER_ID", "unknown")

def main():
    # Wait for shm file
    for _ in range(50):
        if os.path.exists(SHM_PATH) and os.path.getsize(SHM_PATH) > 0:
            break
        time.sleep(0.2)
    else:
        print(f"[{CONTAINER_ID}] SHM file never appeared or is empty!", flush=True)
        sys.exit(1)

    f = open(SHM_PATH, "r+b")
    size = os.path.getsize(SHM_PATH)
    mm = mmap.mmap(f.fileno(), size)

    print(f"[{CONTAINER_ID}] SHM monitor started, file size={size}", flush=True)

    last_hash = None
    change_count = 0

    for tick in range(80):
        # Read current contents via mmap
        mm.seek(0)
        data = mm.read(size)
        h = hashlib.md5(data).hexdigest()[:12]

        if h != last_hash:
            change_count += 1
            print(f"[{CONTAINER_ID}] tick={tick} shm hash CHANGED: {h}", flush=True)
            last_hash = h
        time.sleep(0.1)

    print(f"\n[{CONTAINER_ID}] === SHM MONITOR SUMMARY ===", flush=True)
    print(f"[{CONTAINER_ID}] Total hash changes observed: {change_count}", flush=True)
    if change_count <= 1:
        print(f"[{CONTAINER_ID}] CRITICAL: SHM contents never changed! mmap is NOT shared across containers.", flush=True)
    else:
        print(f"[{CONTAINER_ID}] SHM contents changed {change_count} times - mmap appears to be working.", flush=True)

    mm.close()
    f.close()

if __name__ == "__main__":
    main()
