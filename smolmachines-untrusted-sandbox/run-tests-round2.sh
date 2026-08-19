#!/usr/bin/env bash
# Round 2: follow-ups from the first battery run.
#  R1: disk bomb vs --storage (round 1 showed --overlay does NOT bound "/")
#  R2: HTTP API exec timeout with correct camelCase field (timeoutSecs)
#  R3: precise mount inventory inside the guest (what is actually shared?)
#  R4: full error output for registry-image-without-net on `machine run`
set -u
PASS=0; FAIL=0
report() { local name="$1" status="$2"; shift 2; echo "::: RESULT $status $name — $*";
  if [ "$status" = PASS ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); fi; }
now_ms() { date +%s%3N; }

WORK=$(mktemp -d); trap 'rm -rf "$WORK"' EXIT; cd "$WORK"
docker pull -q alpine:3.20 >/dev/null
docker save alpine:3.20 -o alpine.tar

echo "=== R1: disk bomb vs --storage 3 ==="
df_before=$(df --output=avail -k / | tail -1)
out=$(smolvm machine run --timeout 120s --storage 3 --image ./alpine.tar -- \
    sh -c 'dd if=/dev/zero of=/bigfile bs=1M count=4096 2>&1 | tail -2; df -h / | tail -1' 2>&1)
rc=$?
df_after=$(df --output=avail -k / | tail -1)
host_delta_mb=$(( (df_before - df_after) / 1024 ))
echo "$out"
if echo "$out" | grep -qi 'no space'; then
    report R1-storage-cap PASS "ENOSPC with --storage 3; host avail delta ${host_delta_mb}MB"
else
    report R1-storage-cap FAIL "rc=$rc host_delta=${host_delta_mb}MB"
fi

echo "=== R2: HTTP API exec with camelCase timeoutSecs ==="
smolvm machine create --name sbx2 --image ./alpine.tar >/dev/null 2>&1
smolvm machine start --name sbx2 >/dev/null 2>&1
smolvm serve start --listen 127.0.0.1:8199 &
sleep 2
t0=$(now_ms)
resp=$(curl -s --max-time 60 -X POST 127.0.0.1:8199/api/v1/machines/sbx2/exec \
    -H 'Content-Type: application/json' \
    -d '{"command":["sh","-c","while true; do :; done"],"timeoutSecs":5}')
t1=$(now_ms)
spin_s=$(( (t1-t0)/1000 ))
echo "spin response after ${spin_s}s: $resp"
resp2=$(curl -s --max-time 30 -X POST 127.0.0.1:8199/api/v1/machines/sbx2/exec \
    -H 'Content-Type: application/json' \
    -d '{"command":["echo","still-alive"],"timeoutSecs":10}')
smolvm serve stop >/dev/null 2>&1 || kill %1 2>/dev/null
if [ $spin_s -ge 4 ] && [ $spin_s -le 30 ] && echo "$resp2" | grep -q still-alive; then
    report R2-api-timeout PASS "timeoutSecs killed spin in ${spin_s}s; machine healthy after"
else
    report R2-api-timeout FAIL "spin_s=${spin_s} resp=$resp resp2=$resp2"
fi
smolvm machine stop --name sbx2 >/dev/null 2>&1
smolvm machine delete --name sbx2 -f >/dev/null 2>&1

echo "=== R3: guest mount inventory with -v in:ro out:rw ==="
mkdir -p in out; echo hi > in/x.txt
out3=$(smolvm machine run --timeout 60s --image ./alpine.tar \
    -v "$WORK/in:/in:ro" -v "$WORK/out:/out" -- \
    sh -c 'echo "--- virtiofs mounts:"; grep virtiofs /proc/mounts; echo "--- all mounts:"; cat /proc/mounts | awk "{print \$1, \$2, \$3, \$4}"' 2>&1)
echo "$out3"
shared=$(echo "$out3" | sed -n '/virtiofs mounts:/,/all mounts:/p' | grep -c virtiofs || true)
if echo "$out3" | grep virtiofs | grep -q "/in" && echo "$out3" | grep virtiofs | grep -q "/out"; then
    report R3-mounts PASS "virtiofs shares present for /in and /out ($shared virtiofs mounts total — see list above for what else crosses the boundary)"
else
    report R3-mounts FAIL "see mount list above"
fi

echo "=== R4: full output of registry image + no --net on machine run ==="
out4=$(smolvm machine run --image alpine:3.20 -- echo should-not-run 2>&1)
rc=$?
echo "rc=$rc"
echo "$out4"
if [ $rc -ne 0 ] && ! echo "$out4" | grep -q should-not-run; then
    report R4-no-net-pull PASS "run failed rc=$rc without executing workload (full output above)"
else
    report R4-no-net-pull FAIL "rc=$rc — workload ran or unexpected success"
fi

echo "==================== SUMMARY ===================="
echo "PASS=$PASS FAIL=$FAIL"
exit 0
