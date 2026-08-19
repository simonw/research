#!/usr/bin/env bash
# sandbox-run.sh — run an untrusted Python or JavaScript task under smolvm
# with hard resource limits, no network, and access to designated files only.
#
# Usage:
#   ./sandbox-run.sh --lang python|node --in DIR --out DIR [options] -- CMD ARGS...
#
# Options:
#   --lang LANG        python or node (picks the image tar)
#   --in DIR           host dir mounted read-only at /in
#   --out DIR          host dir mounted read-write at /out (results land here)
#   --mem MiB          memory cap (default 512)
#   --cpus N           vCPU cap (default 1)
#   --timeout DUR      kill the task after DUR (default 30s) e.g. 30s, 5m
#   --overlay GiB      scratch-disk cap for guest writes (default 1)
#   --images DIR       where python.tar / node.tar live (default ./images)
#
# One-time image prep (the only step that needs network, done on the HOST):
#   docker pull python:3.12-alpine && docker save python:3.12-alpine -o images/python.tar
#   docker pull node:22-alpine    && docker save node:22-alpine    -o images/node.tar
#
# Every task run is then fully offline: the guest VM has NO network device.
set -euo pipefail

LANG_CHOICE="" IN_DIR="" OUT_DIR="" MEM=512 CPUS=1 TIMEOUT=30s OVERLAY=1
IMAGES_DIR="$(dirname "$0")/images"

while [ $# -gt 0 ]; do
    case "$1" in
        --lang)    LANG_CHOICE="$2"; shift 2 ;;
        --in)      IN_DIR="$2"; shift 2 ;;
        --out)     OUT_DIR="$2"; shift 2 ;;
        --mem)     MEM="$2"; shift 2 ;;
        --cpus)    CPUS="$2"; shift 2 ;;
        --timeout) TIMEOUT="$2"; shift 2 ;;
        --overlay) OVERLAY="$2"; shift 2 ;;
        --images)  IMAGES_DIR="$2"; shift 2 ;;
        --)        shift; break ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done

case "$LANG_CHOICE" in
    python) IMAGE="$IMAGES_DIR/python.tar" ;;
    node)   IMAGE="$IMAGES_DIR/node.tar" ;;
    *) echo "--lang must be python or node" >&2; exit 2 ;;
esac
[ -f "$IMAGE" ] || { echo "missing image tar: $IMAGE (see prep in header)" >&2; exit 2; }
[ -d "$IN_DIR" ] || { echo "--in must be an existing directory" >&2; exit 2; }
mkdir -p "$OUT_DIR"

# Belt and braces: an outer host-side timeout 60s past the guest one, in case
# the guest agent itself is wedged. `machine run` is ephemeral — the VM and
# overlay are discarded on exit either way.
OUTER=$(( $(echo "$TIMEOUT" | sed -E 's/([0-9]+)s/\1/; s/([0-9]+)m/\1*60/; s/([0-9]+)h/\1*3600/' | bc) + 60 ))

exec timeout --kill-after=10 "$OUTER" \
    smolvm machine run \
    --image "$IMAGE" \
    --cpus "$CPUS" \
    --mem "$MEM" \
    --overlay "$OVERLAY" \
    --timeout "$TIMEOUT" \
    --unprivileged \
    -v "$(realpath "$IN_DIR"):/in:ro" \
    -v "$(realpath "$OUT_DIR"):/out" \
    -- "$@"
