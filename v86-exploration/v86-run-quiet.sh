#!/bin/bash
# v86-run-quiet.sh - Run a command in v86 and extract just the output line
# Usage: ./v86-run-quiet.sh 'command'
# Strips the rodney status messages and echoed command, returning only the result.

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 'command'" >&2
    exit 1
fi

COMMAND="$1"
WAIT="${2:-5}"

uvx rodney clear '#command-input' 2>/dev/null
uvx rodney input '#command-input' "$COMMAND" 2>/dev/null
uvx rodney click '#run-btn' 2>/dev/null
sleep "$WAIT"

# Get the full output, find the echoed command line, print everything after it
uvx rodney text '#output' 2>&1 | awk -v cmd="\$ $COMMAND" '
    found { print }
    $0 == cmd { found=1 }
' | head -20
