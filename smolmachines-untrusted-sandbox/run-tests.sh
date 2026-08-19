#!/usr/bin/env bash
# smolvm untrusted-code sandbox test battery.
# Designed to run on a KVM-capable Linux host (e.g. a GitHub Actions ubuntu runner).
# Every sandboxed execution below runs WITHOUT --net; images are delivered as
# local `docker save` tars so no guest ever needs network access.
set -u

PASS=0
FAIL=0
RESULTS=()

report() { # report NAME PASS|FAIL detail...
    local name="$1" status="$2"; shift 2
    RESULTS+=("$status  $name  $*")
    echo "::: RESULT $status $name — $*"
    if [ "$status" = PASS ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); fi
}

now_ms() { date +%s%3N; }

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

echo "=== environment ==="
uname -a
ls -la /dev/kvm || true
smolvm --version
nproc; free -m | head -2; df -h / | tail -1

echo "=== fetch images offline (docker save) ==="
docker pull -q alpine:3.20
docker pull -q python:3.12-alpine
docker pull -q node:22-alpine
docker save alpine:3.20 -o alpine.tar
docker save python:3.12-alpine -o python.tar
docker save node:22-alpine -o node.tar
ls -la ./*.tar

############################################################
echo "=== T1: cold-boot latency, offline alpine tar, no net ==="
T1_TIMES=()
for i in 1 2 3 4 5; do
    t0=$(now_ms)
    out=$(smolvm machine run --image ./alpine.tar -- echo boot-ok 2>&1)
    t1=$(now_ms)
    T1_TIMES+=($((t1-t0)))
    echo "run $i: $((t1-t0)) ms — ${out##*$'\n'}"
done
if echo "$out" | grep -q boot-ok; then
    report T1-boot PASS "5 cold runs, ms: ${T1_TIMES[*]}"
else
    report T1-boot FAIL "output: $out"
fi

############################################################
echo "=== T2: Python hello, no net ==="
out=$(smolvm machine run --image ./python.tar -- python3 -c 'print(21*2)' 2>&1)
if [ "$(echo "$out" | tail -1)" = "42" ]; then
    report T2-python PASS "python3 prints 42"
else
    report T2-python FAIL "output: $out"
fi

############################################################
echo "=== T3: Node hello, no net ==="
out=$(smolvm machine run --image ./node.tar -- node -e 'console.log(6*7)' 2>&1)
if [ "$(echo "$out" | tail -1)" = "42" ]; then
    report T3-node PASS "node prints 42"
else
    report T3-node FAIL "output: $out"
fi

############################################################
echo "=== T4: network is really off ==="
out=$(smolvm machine run --timeout 30s --image ./alpine.tar -- \
    sh -c 'wget -T 5 -q -O /dev/null https://example.com && echo NET-REACHABLE || echo NET-BLOCKED; nslookup example.com >/dev/null 2>&1 && echo DNS-WORKS || echo DNS-BLOCKED' 2>&1)
if echo "$out" | grep -q NET-BLOCKED && echo "$out" | grep -q DNS-BLOCKED; then
    report T4-no-net PASS "wget and DNS both blocked without --net"
else
    report T4-no-net FAIL "output: $out"
fi

############################################################
echo "=== T4b: registry image without --net is refused up front ==="
out=$(smolvm machine run --image alpine:3.20 -- echo should-not-run 2>&1)
rc=$?
if [ $rc -ne 0 ] && ! echo "$out" | grep -q should-not-run; then
    report T4b-pull-refusal PASS "refused, rc=$rc: $(echo "$out" | head -2 | tr '\n' ' ')"
else
    report T4b-pull-refusal FAIL "rc=$rc output: $out"
fi

############################################################
echo "=== T5: while-true CPU spin killed by --timeout ==="
t0=$(now_ms)
out=$(smolvm machine run --timeout 10s --cpus 1 --image ./python.tar -- \
    python3 -c 'while True: pass' 2>&1)
rc=$?
t1=$(now_ms)
elapsed=$(( (t1-t0)/1000 ))
sleep 2
leftovers=$(pgrep -af 'smolvm|krun' | grep -v 'run-tests' | grep -vc 'pgrep' || true)
if [ $rc -ne 0 ] && [ $elapsed -lt 60 ] && [ "${leftovers:-0}" -eq 0 ]; then
    report T5-cpu-spin PASS "rc=$rc after ${elapsed}s, no leftover VMM processes"
else
    report T5-cpu-spin FAIL "rc=$rc elapsed=${elapsed}s leftovers=$leftovers output: $(echo "$out" | tail -2)"
fi

############################################################
echo "=== T6: memory bomb contained by --mem 256 ==="
host_avail_before=$(awk '/MemAvailable/{print $2}' /proc/meminfo)
out=$(smolvm machine run --timeout 60s --mem 256 --image ./python.tar -- \
    python3 -c 'a = bytearray(1024*1024*1024); print("ALLOCATED-1GB")' 2>&1)
rc=$?
host_avail_after=$(awk '/MemAvailable/{print $2}' /proc/meminfo)
if [ $rc -ne 0 ] && ! echo "$out" | grep -q ALLOCATED-1GB; then
    report T6-mem-bomb PASS "1GiB alloc failed in 256MiB VM (rc=$rc); host MemAvailable ${host_avail_before}->${host_avail_after} kB"
else
    report T6-mem-bomb FAIL "rc=$rc output: $(echo "$out" | tail -3)"
fi

############################################################
echo "=== T7: fork bomb contained (--cpus 1, --timeout 20s) ==="
t0=$(now_ms)
out=$(smolvm machine run --timeout 20s --cpus 1 --mem 256 --image ./alpine.tar -- \
    sh -c ':(){ :|:& };:; sleep 60' 2>&1)
rc=$?
t1=$(now_ms)
elapsed=$(( (t1-t0)/1000 ))
load=$(cut -d' ' -f1 /proc/loadavg)
sleep 2
leftovers=$(pgrep -af 'smolvm|krun' | grep -v 'run-tests' | grep -vc 'pgrep' || true)
if [ $elapsed -lt 90 ] && [ "${leftovers:-0}" -eq 0 ]; then
    report T7-fork-bomb PASS "returned in ${elapsed}s (rc=$rc), host load ${load}, no leftovers"
else
    report T7-fork-bomb FAIL "elapsed=${elapsed}s rc=$rc load=$load leftovers=$leftovers"
fi

############################################################
echo "=== T8: disk bomb contained by --overlay 1 ==="
df_before=$(df --output=avail -k / | tail -1)
out=$(smolvm machine run --timeout 120s --overlay 1 --image ./alpine.tar -- \
    sh -c 'dd if=/dev/zero of=/bigfile bs=1M count=4096 2>&1; df -h / | tail -1' 2>&1)
rc=$?
df_after=$(df --output=avail -k / | tail -1)
host_delta_mb=$(( (df_before - df_after) / 1024 ))
if echo "$out" | grep -qi 'no space\|error'; then
    report T8-disk-bomb PASS "guest hit ENOSPC; host / shrank by ${host_delta_mb}MB during test"
else
    report T8-disk-bomb FAIL "rc=$rc host_delta=${host_delta_mb}MB output: $(echo "$out" | tail -3)"
fi

############################################################
echo "=== T9: filesystem — designated dirs only, ro enforced ==="
mkdir -p in out
echo "id,value" > in/data.csv
echo "1,hello" >> in/data.csv
out=$(smolvm machine run --timeout 60s --image ./alpine.tar \
    -v "$WORK/in:/in:ro" -v "$WORK/out:/out" -- \
    sh -c 'cat /in/data.csv >/dev/null && echo READ-OK;
           echo tamper > /in/data.csv 2>/dev/null && echo RO-BREACH || echo RO-ENFORCED;
           echo result > /out/result.txt && echo WRITE-OK;
           ls /root /home 2>/dev/null | grep -q . && echo HOST-FILES-VISIBLE || echo GUEST-CLEAN' 2>&1)
if echo "$out" | grep -q READ-OK && echo "$out" | grep -q RO-ENFORCED \
   && echo "$out" | grep -q WRITE-OK && [ "$(cat out/result.txt 2>/dev/null)" = "result" ]; then
    report T9-fs-isolation PASS "ro mount enforced, rw output round-trips, $(echo "$out" | grep -o 'GUEST-CLEAN\|HOST-FILES-VISIBLE')"
else
    report T9-fs-isolation FAIL "output: $out ; host out/: $(ls out 2>&1)"
fi

############################################################
echo "=== T10: end-to-end data transformation demo (Python + Node) ==="
cat > in/transform.py <<'EOF'
import csv, json
with open('/in/data.csv') as f:
    rows = list(csv.DictReader(f))
with open('/out/data.json', 'w') as f:
    json.dump(rows, f)
print(f"transformed {len(rows)} rows")
EOF
out=$(smolvm machine run --timeout 60s --mem 512 --cpus 1 --image ./python.tar \
    -v "$WORK/in:/in:ro" -v "$WORK/out:/out" -- python3 /in/transform.py 2>&1)
py_ok=false
grep -q '"value": "hello"' out/data.json 2>/dev/null && py_ok=true
cat > in/transform.js <<'EOF'
const fs = require('fs');
const rows = JSON.parse(fs.readFileSync('/in/data.json.copy', 'utf8'));
fs.writeFileSync('/out/data.tsv', rows.map(r => Object.values(r).join('\t')).join('\n'));
console.log(`wrote ${rows.length} rows`);
EOF
cp out/data.json in/data.json.copy 2>/dev/null || echo '[]' > in/data.json.copy
out2=$(smolvm machine run --timeout 60s --mem 512 --cpus 1 --image ./node.tar \
    -v "$WORK/in:/in:ro" -v "$WORK/out:/out" -- node /in/transform.js 2>&1)
node_ok=false
grep -q "hello" out/data.tsv 2>/dev/null && node_ok=true
if $py_ok && $node_ok; then
    report T10-transform PASS "CSV->JSON (python) and JSON->TSV (node) with ro input, rw output, no net"
else
    report T10-transform FAIL "py_ok=$py_ok node_ok=$node_ok out1:$out out2:$out2"
fi

############################################################
echo "=== T11: persistent machine — warm exec latency ==="
smolvm machine create --name sbx --image ./python.tar --mem 512 --cpus 2 \
    -v "$WORK/in:/in:ro" -v "$WORK/out:/out" 2>&1 | tail -2
t0=$(now_ms); smolvm machine start --name sbx 2>&1 | tail -1; t1=$(now_ms)
start_ms=$((t1-t0))
EXEC_TIMES=()
warm_out=""
for i in 1 2 3 4 5; do
    t0=$(now_ms)
    warm_out=$(smolvm machine exec --name sbx -- python3 -c 'print("warm")' 2>&1)
    t1=$(now_ms)
    EXEC_TIMES+=($((t1-t0)))
done
t0=$(now_ms)
smolvm machine exec --timeout 5s --name sbx -- python3 -c 'while True: pass' >/dev/null 2>&1
rc=$?
t1=$(now_ms)
spin_s=$(( (t1-t0)/1000 ))
post_spin=$(smolvm machine exec --name sbx -- python3 -c 'print("alive")' 2>&1 | tail -1)
if echo "$warm_out" | grep -q warm && [ "$post_spin" = "alive" ] && [ $spin_s -lt 30 ]; then
    report T11-warm-pool PASS "start ${start_ms}ms; 5 warm execs ms: ${EXEC_TIMES[*]}; exec --timeout killed spin in ${spin_s}s (rc=$rc), machine still healthy"
else
    report T11-warm-pool FAIL "warm=$warm_out post_spin=$post_spin spin_s=$spin_s"
fi

############################################################
echo "=== T12: HTTP API (serve) — exec with timeout_secs, file up/download ==="
smolvm serve start --listen 127.0.0.1:8199 &
SERVE_PID=$!
sleep 2
api_out=$(curl -s -X POST 127.0.0.1:8199/api/v1/machines/sbx/exec \
    -H 'Content-Type: application/json' \
    -d '{"command":["python3","-c","print(40+2)"],"timeout_secs":30}')
echo "exec response: $api_out"
curl -s -X PUT 127.0.0.1:8199/api/v1/machines/sbx/files/workspace/api-upload.txt \
    --data-binary 'uploaded-via-api' > /dev/null
dl=$(curl -s 127.0.0.1:8199/api/v1/machines/sbx/files/workspace/api-upload.txt)
t0=$(now_ms)
api_spin=$(curl -s -X POST 127.0.0.1:8199/api/v1/machines/sbx/exec \
    -H 'Content-Type: application/json' \
    -d '{"command":["sh","-c","while true; do :; done"],"timeout_secs":5}')
t1=$(now_ms)
api_spin_s=$(( (t1-t0)/1000 ))
kill $SERVE_PID 2>/dev/null; smolvm serve stop 2>/dev/null || true
if echo "$api_out" | grep -q 42 && [ "$dl" = "uploaded-via-api" ] && [ $api_spin_s -lt 30 ]; then
    report T12-http-api PASS "exec ok, file round-trip ok, timeout_secs killed spin in ${api_spin_s}s: $(echo "$api_spin" | head -c 120)"
else
    report T12-http-api FAIL "exec=$api_out dl=$dl spin_s=$api_spin_s spin=$api_spin"
fi
smolvm machine stop --name sbx 2>/dev/null
smolvm machine delete --name sbx -f 2>/dev/null

############################################################
echo "=== T13: --unprivileged drops capabilities ==="
priv=$(smolvm machine run --timeout 60s --image ./alpine.tar -- \
    sh -c 'grep CapEff /proc/self/status' 2>&1 | tail -1)
unpriv=$(smolvm machine run --timeout 60s --unprivileged --image ./alpine.tar -- \
    sh -c 'grep CapEff /proc/self/status' 2>&1 | tail -1)
echo "default:      $priv"
echo "unprivileged: $unpriv"
if [ -n "$unpriv" ] && [ "$priv" != "$unpriv" ]; then
    report T13-unprivileged PASS "CapEff default='$priv' vs unprivileged='$unpriv'"
else
    report T13-unprivileged FAIL "default='$priv' unprivileged='$unpriv'"
fi

############################################################
echo ""
echo "==================== SUMMARY ===================="
for r in "${RESULTS[@]}"; do echo "$r"; done
echo "PASS=$PASS FAIL=$FAIL"
exit 0
