#!/bin/bash
# v86-lua.sh - Run a Lua script in the v86 Linux Emulator
# Usage: ./v86-lua.sh 'lua expression or script'
# Example: ./v86-lua.sh 'for i=1,10 do print(i*i) end'

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 'lua code'" >&2
    exit 1
fi

LUA_CODE="$1"
WAIT="${2:-5}"

uvx rodney clear '#command-input' 2>/dev/null
uvx rodney input '#command-input' "lua -e '$LUA_CODE'" 2>/dev/null
uvx rodney click '#run-btn' 2>/dev/null
sleep "$WAIT"
uvx rodney text '#output' 2>&1
