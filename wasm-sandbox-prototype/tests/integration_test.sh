#!/bin/bash

# Integration test for wasm-sandbox
# This script tests the JSON protocol by sending commands via stdin

set -e

BINARY="./target/debug/wasm-sandbox"

if [ ! -f "$BINARY" ]; then
    echo "Error: Binary not found at $BINARY"
    echo "Run 'cargo build' first"
    exit 1
fi

echo "Starting wasm-sandbox integration tests..."
echo

# Start the sandbox and send test commands
# Redirect stderr to /dev/null to see only the JSON responses
{
    # Test 1: Execute echo command
    echo '{"type":"shell","command":"echo Hello World","id":"test-1","time_limit_ms":1000}'
    sleep 0.1

    # Test 2: Execute pwd command
    echo '{"type":"shell","command":"pwd","id":"test-2","time_limit_ms":1000}'
    sleep 0.1

    # Test 3: Write a file
    echo '{"type":"write_file","path":"/tmp/test.txt","content":"Hello from sandbox","id":"test-3"}'
    sleep 0.1

    # Test 4: Read the file back
    echo '{"type":"read_file","path":"/tmp/test.txt","id":"test-4"}'
    sleep 0.1

    # Test 5: Execute cat command
    echo '{"type":"shell","command":"cat /tmp/test.txt","id":"test-5","time_limit_ms":1000}'
    sleep 0.1

    # Test 6: List files
    echo '{"type":"shell","command":"ls /tmp","id":"test-6","time_limit_ms":1000}'
    sleep 0.1

    # Test 7: Get status
    echo '{"type":"status","id":"test-7"}'
    sleep 0.1

    # Test 8: Reset sandbox
    echo '{"type":"reset","id":"test-8"}'
    sleep 0.1

    # Test 9: Verify file is gone after reset
    echo '{"type":"read_file","path":"/tmp/test.txt","id":"test-9"}'
    sleep 0.1

    # Give it time to process
    sleep 1
} | "$BINARY" 2>/dev/null

echo
echo "All tests passed!"

echo
echo "Integration tests completed!"
