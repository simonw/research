#!/bin/bash
# Integration test for rod-cli
set -e

PASS=0
FAIL=0
CLI="./rod-cli"

pass() { echo "  PASS: $1"; PASS=$((PASS + 1)); }
fail() { echo "  FAIL: $1 - got: $2"; FAIL=$((FAIL + 1)); }

assert_eq() {
    if [ "$1" = "$2" ]; then
        pass "$3"
    else
        fail "$3" "'$1' != '$2'"
    fi
}

assert_contains() {
    if echo "$1" | grep -q "$2"; then
        pass "$3"
    else
        fail "$3" "'$1' does not contain '$2'"
    fi
}

assert_exit() {
    if [ "$1" -eq "$2" ]; then
        pass "$3"
    else
        fail "$3" "exit $1 != $2"
    fi
}

echo "=== rod-cli integration tests ==="
echo ""

# Start a test server
/tmp/testserver &
SERVER_PID=$!
sleep 2

cleanup() {
    $CLI stop 2>/dev/null || true
    kill $SERVER_PID 2>/dev/null || true
}
trap cleanup EXIT

# --- Browser lifecycle ---
echo "[Browser lifecycle]"

OUT=$($CLI start 2>&1)
assert_contains "$OUT" "Chrome started" "start launches Chrome"

OUT=$($CLI status 2>&1)
assert_contains "$OUT" "Browser running" "status shows running"

# --- Navigation ---
echo "[Navigation]"

OUT=$($CLI open http://127.0.0.1:18080/ 2>&1)
assert_eq "$OUT" "Test Page" "open returns page title"

OUT=$($CLI url 2>&1)
assert_eq "$OUT" "http://127.0.0.1:18080/" "url returns current URL"

OUT=$($CLI title 2>&1)
assert_eq "$OUT" "Test Page" "title returns page title"

OUT=$($CLI reload 2>&1)
assert_eq "$OUT" "Reloaded" "reload works"

# --- Element queries ---
echo "[Element queries]"

OUT=$($CLI text h1 2>&1)
assert_eq "$OUT" "Hello Rod CLI" "text extracts element text"

OUT=$($CLI html h1 2>&1)
assert_eq "$OUT" "<h1>Hello Rod CLI</h1>" "html extracts element HTML"

OUT=$($CLI attr "#link1" href 2>&1)
assert_eq "$OUT" "/page2" "attr extracts attribute"

OUT=$($CLI count ".item" 2>&1)
assert_eq "$OUT" "3" "count returns element count"

OUT=$($CLI exists "#btn1" 2>&1)
assert_eq "$OUT" "true" "exists returns true for existing"

OUT=$($CLI exists "#nope" 2>&1) || true
assert_eq "$OUT" "false" "exists returns false for missing"

OUT=$($CLI visible "#info" 2>&1)
assert_eq "$OUT" "true" "visible returns true"

OUT=$($CLI visible "#hidden" 2>&1) || true
assert_eq "$OUT" "false" "visible returns false for hidden"

# --- JavaScript ---
echo "[JavaScript]"

OUT=$($CLI js 'document.title' 2>&1)
assert_eq "$OUT" "Test Page" "js evaluates expressions"

OUT=$($CLI js '1 + 2 + 3' 2>&1)
assert_eq "$OUT" "6" "js evaluates math"

OUT=$($CLI js 'null' 2>&1)
assert_eq "$OUT" "null" "js handles null"

OUT=$($CLI js 'true' 2>&1)
assert_eq "$OUT" "true" "js handles booleans"

OUT=$($CLI js '[1,2,3]' 2>&1)
assert_contains "$OUT" "1" "js handles arrays"

# --- Interaction ---
echo "[Interaction]"

$CLI open http://127.0.0.1:18080/ >/dev/null 2>&1

OUT=$($CLI click "#btn1" 2>&1)
assert_eq "$OUT" "Clicked" "click reports success"

OUT=$($CLI text "#info" 2>&1)
assert_eq "$OUT" "Clicked!" "click actually changed DOM"

OUT=$($CLI input "#search" "test input" 2>&1)
assert_contains "$OUT" "Typed" "input reports success"

OUT=$($CLI js 'document.querySelector("#search").value' 2>&1)
assert_eq "$OUT" "test input" "input actually typed text"

OUT=$($CLI clear "#search" 2>&1)
assert_eq "$OUT" "Cleared" "clear reports success"

OUT=$($CLI select "#color" "green" 2>&1)
assert_contains "$OUT" "green" "select changes value"

# --- Waiting ---
echo "[Waiting]"

OUT=$($CLI waitload 2>&1)
assert_eq "$OUT" "Page loaded" "waitload works"

OUT=$($CLI waitstable 2>&1)
assert_eq "$OUT" "DOM stable" "waitstable works"

OUT=$($CLI wait "#info" 2>&1)
assert_eq "$OUT" "Element visible" "wait for element works"

# --- Screenshots ---
echo "[Screenshots]"

OUT=$($CLI screenshot /tmp/rod-test-ss.png 2>&1)
assert_contains "$OUT" "Saved" "screenshot saves file"
test -f /tmp/rod-test-ss.png && pass "screenshot file exists" || fail "screenshot file exists" "missing"

# --- Tabs ---
echo "[Tabs]"

OUT=$($CLI pages 2>&1)
assert_contains "$OUT" "[0]" "pages lists tabs"

OUT=$($CLI newpage http://127.0.0.1:18080/page2 2>&1)
assert_contains "$OUT" "Opened" "newpage opens tab"

OUT=$($CLI title 2>&1)
assert_eq "$OUT" "Page 2" "new tab is active"

OUT=$($CLI page 0 2>&1)
assert_contains "$OUT" "Switched" "page switches tab"

OUT=$($CLI title 2>&1)
assert_eq "$OUT" "Test Page" "switched back to first tab"

OUT=$($CLI closepage 1 2>&1)
assert_contains "$OUT" "Closed" "closepage closes tab"

# --- Cleanup ---
echo "[Cleanup]"

OUT=$($CLI stop 2>&1)
assert_eq "$OUT" "Chrome stopped" "stop shuts down Chrome"

OUT=$($CLI status 2>&1)
assert_contains "$OUT" "No active" "status shows no session"

# --- Summary ---
echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
if [ $FAIL -gt 0 ]; then
    exit 1
fi
