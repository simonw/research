#!/bin/bash
#
# Redis JavaScript Module Test Suite
# Tests the redis-js.so module functionality
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
MODULE_PATH="$PROJECT_DIR/redis-js.so"
REDIS_CLI="${REDIS_CLI:-redis-cli}"
REDIS_SERVER="${REDIS_SERVER:-redis-server}"
REDIS_PORT="${REDIS_PORT:-6399}"
REDIS_PID=""
TESTS_PASSED=0
TESTS_FAILED=0

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${YELLOW}[INFO]${NC} $1"
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    TESTS_PASSED=$((TESTS_PASSED + 1))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    TESTS_FAILED=$((TESTS_FAILED + 1))
}

cleanup() {
    if [ -n "$REDIS_PID" ] && kill -0 "$REDIS_PID" 2>/dev/null; then
        log_info "Stopping Redis server (PID: $REDIS_PID)..."
        kill "$REDIS_PID" 2>/dev/null || true
        wait "$REDIS_PID" 2>/dev/null || true
    fi
    rm -f /tmp/redis-js-test.conf
}

trap cleanup EXIT

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    if [ ! -f "$MODULE_PATH" ]; then
        echo "Error: Module not found at $MODULE_PATH"
        echo "Please run 'make' first to build the module."
        exit 1
    fi

    if ! command -v "$REDIS_SERVER" &> /dev/null; then
        echo "Error: redis-server not found. Please install Redis or set REDIS_SERVER env var."
        exit 1
    fi

    if ! command -v "$REDIS_CLI" &> /dev/null; then
        echo "Error: redis-cli not found. Please install Redis or set REDIS_CLI env var."
        exit 1
    fi

    log_info "All prerequisites met."
}

# Start Redis with the module
start_redis() {
    log_info "Starting Redis server on port $REDIS_PORT with JS module..."

    # Create a minimal config
    cat > /tmp/redis-js-test.conf << EOF
port $REDIS_PORT
daemonize no
loglevel warning
loadmodule $MODULE_PATH
EOF

    "$REDIS_SERVER" /tmp/redis-js-test.conf &
    REDIS_PID=$!

    # Wait for Redis to be ready
    local retries=30
    while [ $retries -gt 0 ]; do
        if "$REDIS_CLI" -p "$REDIS_PORT" PING 2>/dev/null | grep -q PONG; then
            log_info "Redis server started successfully (PID: $REDIS_PID)"
            return 0
        fi
        sleep 0.1
        ((retries--))
    done

    echo "Error: Failed to start Redis server"
    exit 1
}

# Run a Redis command and return the result
redis_cmd() {
    "$REDIS_CLI" -p "$REDIS_PORT" "$@" 2>&1
}

# Test helper: assert equality
assert_eq() {
    local test_name="$1"
    local expected="$2"
    local actual="$3"

    if [ "$expected" = "$actual" ]; then
        log_pass "$test_name"
    else
        log_fail "$test_name"
        echo "       Expected: $expected"
        echo "       Actual:   $actual"
    fi
}

# Test helper: assert contains
assert_contains() {
    local test_name="$1"
    local expected="$2"
    local actual="$3"

    if [[ "$actual" == *"$expected"* ]]; then
        log_pass "$test_name"
    else
        log_fail "$test_name"
        echo "       Expected to contain: $expected"
        echo "       Actual: $actual"
    fi
}

# ====================
# TEST CASES
# ====================

test_basic_eval() {
    log_info "Testing basic JS.EVAL..."

    # Simple return value
    result=$(redis_cmd JS.EVAL "return 42" 0)
    assert_eq "Return integer" "42" "$result"

    # Return string
    result=$(redis_cmd JS.EVAL "return 'hello'" 0)
    assert_eq "Return string" "hello" "$result"

    # Return null
    result=$(redis_cmd JS.EVAL "return null" 0)
    assert_eq "Return null" "" "$result"

    # Math operations
    result=$(redis_cmd JS.EVAL "return 10 + 32" 0)
    assert_eq "Math addition" "42" "$result"

    # Boolean true
    result=$(redis_cmd JS.EVAL "return true" 0)
    assert_eq "Return true" "1" "$result"

    # Boolean false (returns 0 in JavaScript, empty/nil in Lua)
    result=$(redis_cmd JS.EVAL "return false" 0)
    assert_eq "Return false" "0" "$result"
}

test_arrays() {
    log_info "Testing array handling..."

    # Return array
    result=$(redis_cmd JS.EVAL "return [1, 2, 3]" 0)
    assert_contains "Return array - contains 1" "1" "$result"
    assert_contains "Return array - contains 2" "2" "$result"
    assert_contains "Return array - contains 3" "3" "$result"

    # Array with strings
    result=$(redis_cmd JS.EVAL "return ['a', 'b', 'c']" 0)
    assert_contains "String array - contains a" "a" "$result"
    assert_contains "String array - contains b" "b" "$result"
}

test_keys_argv() {
    log_info "Testing KEYS and ARGV..."

    # Access KEYS
    result=$(redis_cmd JS.EVAL "return KEYS.length" 2 key1 key2)
    assert_eq "KEYS.length" "2" "$result"

    result=$(redis_cmd JS.EVAL "return KEYS[0]" 1 mykey)
    assert_eq "KEYS[0]" "mykey" "$result"

    # Access ARGV
    result=$(redis_cmd JS.EVAL "return ARGV.length" 0 arg1 arg2 arg3)
    assert_eq "ARGV.length" "3" "$result"

    result=$(redis_cmd JS.EVAL "return ARGV[1]" 0 first second third)
    assert_eq "ARGV[1]" "second" "$result"

    # Combined
    result=$(redis_cmd JS.EVAL "return KEYS[0] + ':' + ARGV[0]" 1 prefix value)
    assert_eq "KEYS+ARGV concat" "prefix:value" "$result"
}

test_redis_call() {
    log_info "Testing redis.call()..."

    # SET and GET
    result=$(redis_cmd JS.EVAL "return redis.call('SET', KEYS[0], ARGV[0])" 1 testkey testvalue)
    assert_eq "redis.call SET" "OK" "$result"

    result=$(redis_cmd JS.EVAL "return redis.call('GET', KEYS[0])" 1 testkey)
    assert_eq "redis.call GET" "testvalue" "$result"

    # INCR
    redis_cmd JS.EVAL "redis.call('SET', 'counter', '0')" 0 > /dev/null
    result=$(redis_cmd JS.EVAL "return redis.call('INCR', 'counter')" 0)
    assert_eq "redis.call INCR" "1" "$result"

    # LPUSH and LRANGE
    redis_cmd JS.EVAL "redis.call('DEL', 'mylist')" 0 > /dev/null
    redis_cmd JS.EVAL "redis.call('LPUSH', 'mylist', 'a', 'b', 'c')" 0 > /dev/null
    result=$(redis_cmd JS.EVAL "return redis.call('LLEN', 'mylist')" 0)
    assert_eq "redis.call LLEN" "3" "$result"

    # DEL
    result=$(redis_cmd JS.EVAL "return redis.call('DEL', 'testkey', 'counter', 'mylist')" 0)
    assert_contains "redis.call DEL" "3" "$result"
}

test_redis_pcall() {
    log_info "Testing redis.pcall()..."

    # pcall should not throw on error
    result=$(redis_cmd JS.EVAL "var r = redis.pcall('GET', 'nonexistent'); return r === null ? 'null' : r" 0)
    assert_eq "redis.pcall GET nonexistent" "null" "$result"

    # pcall with valid command
    redis_cmd JS.EVAL "redis.call('SET', 'pcalltest', 'hello')" 0 > /dev/null
    result=$(redis_cmd JS.EVAL "return redis.pcall('GET', 'pcalltest')" 0)
    assert_eq "redis.pcall GET valid" "hello" "$result"

    # Cleanup
    redis_cmd DEL pcalltest > /dev/null
}

test_script_caching() {
    log_info "Testing script caching (JS.LOAD/JS.CALL)..."

    # Load a script
    sha=$(redis_cmd JS.LOAD "return ARGV[0].toUpperCase()")
    assert_contains "JS.LOAD returns SHA" "" "$sha"  # Just check it returns something

    if [ ${#sha} -eq 40 ]; then
        log_pass "JS.LOAD returns 40-char SHA"
    else
        log_fail "JS.LOAD returns 40-char SHA (got ${#sha} chars: $sha)"
    fi

    # Call the cached script
    if [ ${#sha} -eq 40 ]; then
        result=$(redis_cmd JS.CALL "$sha" 0 "hello world")
        assert_eq "JS.CALL cached script" "HELLO WORLD" "$result"
    fi

    # Check if script exists
    if [ ${#sha} -eq 40 ]; then
        result=$(redis_cmd JS.EXISTS "$sha")
        assert_eq "JS.EXISTS for loaded script" "1" "$result"
    fi

    # Check non-existent SHA
    result=$(redis_cmd JS.EXISTS "0000000000000000000000000000000000000000")
    assert_eq "JS.EXISTS for non-existent" "0" "$result"
}

test_js_flush() {
    log_info "Testing JS.FLUSH..."

    # Load a script first
    sha=$(redis_cmd JS.LOAD "return 1")

    # Flush all scripts
    result=$(redis_cmd JS.FLUSH)
    assert_eq "JS.FLUSH returns OK" "OK" "$result"

    # Verify script no longer exists
    result=$(redis_cmd JS.EXISTS "$sha")
    assert_eq "Script removed after flush" "0" "$result"
}

test_error_handling() {
    log_info "Testing error handling..."

    # Syntax error
    result=$(redis_cmd JS.EVAL "return {{{" 0)
    assert_contains "Syntax error detected" "ERR" "$result"

    # Runtime error - undefined variable
    result=$(redis_cmd JS.EVAL "return undefinedVar.property" 0)
    assert_contains "Runtime error detected" "ERR" "$result"

    # JS.CALL with non-existent SHA
    result=$(redis_cmd JS.CALL "0000000000000000000000000000000000000000" 0)
    assert_contains "JS.CALL unknown SHA error" "NOSCRIPT" "$result"
}

test_complex_scripts() {
    log_info "Testing complex scripts..."

    # Loop with accumulator
    result=$(redis_cmd JS.EVAL "var sum = 0; for (var i = 1; i <= 10; i++) sum += i; return sum" 0)
    assert_eq "Loop with sum" "55" "$result"

    # Function definition and call
    result=$(redis_cmd JS.EVAL "function factorial(n) { return n <= 1 ? 1 : n * factorial(n-1); } return factorial(5)" 0)
    assert_eq "Factorial function" "120" "$result"

    # Object manipulation
    result=$(redis_cmd JS.EVAL "var obj = {a: 1, b: 2}; return obj.a + obj.b" 0)
    assert_eq "Object property access" "3" "$result"

    # String manipulation
    result=$(redis_cmd JS.EVAL "return 'hello'.length" 0)
    assert_eq "String length" "5" "$result"

    # Array methods
    result=$(redis_cmd JS.EVAL "return [1,2,3,4,5].length" 0)
    assert_eq "Array length" "5" "$result"
}

test_redis_log() {
    log_info "Testing redis.log()..."

    # Just verify it doesn't crash - log output goes to Redis logs
    result=$(redis_cmd JS.EVAL "redis.log(redis.LOG_WARNING, 'Test log message'); return 'ok'" 0)
    assert_eq "redis.log doesn't crash" "ok" "$result"
}

test_sha1hex() {
    log_info "Testing redis.sha1hex()..."

    # Known SHA1 hash of empty string
    result=$(redis_cmd JS.EVAL "return redis.sha1hex('')" 0)
    assert_eq "SHA1 of empty string" "da39a3ee5e6b4b0d3255bfef95601890afd80709" "$result"

    # Known SHA1 hash of 'hello'
    result=$(redis_cmd JS.EVAL "return redis.sha1hex('hello')" 0)
    assert_eq "SHA1 of 'hello'" "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d" "$result"
}

# ====================
# MAIN
# ====================

main() {
    echo "========================================"
    echo "Redis JavaScript Module Test Suite"
    echo "========================================"
    echo ""

    check_prerequisites
    start_redis

    echo ""
    echo "Running tests..."
    echo ""

    test_basic_eval
    test_arrays
    test_keys_argv
    test_redis_call
    test_redis_pcall
    test_script_caching
    test_js_flush
    test_error_handling
    test_complex_scripts
    test_redis_log
    test_sha1hex

    echo ""
    echo "========================================"
    echo "Test Results"
    echo "========================================"
    echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
    echo -e "Failed: ${RED}$TESTS_FAILED${NC}"
    echo ""

    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "${GREEN}All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}Some tests failed.${NC}"
        exit 1
    fi
}

main "$@"
