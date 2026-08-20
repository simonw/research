#!/bin/bash
# Benchmark the bun-webview-json-api server under a cgroup memory limit.
#
# Usage: bench.sh <limit_mb|0 for none> <chrome_path> [extra chrome args...]
# Prints: PASS/FAIL, peak cgroup memory usage, and per-request results.
set -u
LIMIT_MB=$1; CHROME=$2; shift 2
EXTRA="${*:-}"
DIR=$(cd "$(dirname "$0")" && pwd)
CG=/sys/fs/cgroup/memory/bwv
PORT=8044

pkill -x bun 2>/dev/null; pkill -x chrome 2>/dev/null; pkill -x headless_shell 2>/dev/null
sleep 0.5

mkdir -p $CG
if [ "$LIMIT_MB" -gt 0 ]; then
  echo $((LIMIT_MB*1024*1024)) > $CG/memory.limit_in_bytes
  echo 0 > $CG/memory.swappiness
else
  echo -1 > $CG/memory.limit_in_bytes 2>/dev/null || echo 9223372036854771712 > $CG/memory.limit_in_bytes
fi
echo 0 > $CG/memory.max_usage_in_bytes 2>/dev/null || true

# start test pages (outside the cgroup — not part of the service under test)
if ! curl -s --noproxy '*' -o /dev/null localhost:8055/simple; then
  nohup bun "$DIR/testpages.ts" >/tmp/bwv/testpages.log 2>&1 &
  sleep 1
fi

# launch the API server inside the cgroup
BUN_CHROME_PATH=$CHROME CHROME_EXTRA_ARGS="--no-sandbox $EXTRA" PORT=$PORT \
  bash -c "echo \$\$ > $CG/cgroup.procs; exec bun '$DIR/server.ts'" \
  >/tmp/bwv/bench-server.log 2>&1 &
SRV=$!
sleep 2

ok=0; fail=0
run() {
  local out
  out=$(curl -s --noproxy '*' --max-time 30 -X POST localhost:$PORT/$1 \
        -H 'Content-Type: application/json' -d "$2")
  if echo "$out" | head -c 200 | grep -q '"ok":true\|PNG\|�'; then ok=$((ok+1)); else
    fail=$((fail+1)); echo "  FAIL $1: $(echo "$out" | head -c 120)"; fi
}
runshot() {
  local code
  code=$(curl -s --noproxy '*' --max-time 30 -o /tmp/bwv/bench-shot.png -w "%{http_code}" \
         -X POST localhost:$PORT/screenshot -H 'Content-Type: application/json' -d "$1")
  if [ "$code" = "200" ]; then ok=$((ok+1)); else fail=$((fail+1)); echo "  FAIL screenshot ($code)"; fi
}

for i in 1 2 3; do
  run javascript '{"url":"http://localhost:8055/simple","javascript":"document.title"}'
  run javascript '{"url":"http://localhost:8055/heavy","javascript":"({title:document.title,n:window.bigArray.length})"}'
  runshot '{"url":"http://localhost:8055/heavy","width":1280,"height":800}'
done

peak=$(cat $CG/memory.max_usage_in_bytes 2>/dev/null || echo 0)
kill $SRV 2>/dev/null; pkill -x chrome 2>/dev/null; pkill -x headless_shell 2>/dev/null
echo "limit=${LIMIT_MB}MB ok=$ok fail=$fail peak=$((peak/1024/1024))MB"
[ $fail -eq 0 ] && exit 0 || exit 1
