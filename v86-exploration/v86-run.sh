#!/bin/bash
# v86-run.sh - Run a command in the v86 Linux Emulator
# Usage: ./v86-run.sh 'command to run'
# Requires: uvx rodney (browser automation tool)
#
# The v86 emulator must be open in the browser at:
#   https://tools.simonwillison.net/v86

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 'command'" >&2
    exit 1
fi

COMMAND="$1"
WAIT="${2:-5}"  # seconds to wait for output, default 5

# Clear previous input, type command, click run, wait, read output
uvx rodney clear '#command-input' 2>/dev/null
uvx rodney input '#command-input' "$COMMAND" 2>/dev/null
uvx rodney click '#run-btn' 2>/dev/null
sleep "$WAIT"
uvx rodney text '#output' 2>&1
