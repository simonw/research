#!/bin/bash
# v86-multi.sh - Run multiple commands in v86, separated by semicolons
# Usage: ./v86-multi.sh 'cmd1; cmd2; cmd3'
#
# The v86 emulator runs all commands as a single shell input,
# so semicolons and pipes work naturally.
# Use a longer wait time for complex operations.

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 'cmd1; cmd2; cmd3' [wait_seconds]" >&2
    exit 1
fi

COMMANDS="$1"
WAIT="${2:-8}"

uvx rodney clear '#command-input' 2>/dev/null
uvx rodney input '#command-input' "$COMMANDS" 2>/dev/null
uvx rodney click '#run-btn' 2>/dev/null
sleep "$WAIT"
uvx rodney text '#output' 2>&1
