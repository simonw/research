#!/bin/bash
set -e

echo "Starting docker-chrome-neko..."
echo "CDP Agent port: ${CDP_AGENT_PORT:-3001}"

# Start CDP agent in background
cd /opt/cdp-agent
node index.js &
CDP_PID=$!
echo "CDP agent started (PID: $CDP_PID)"

# Wait a moment for CDP agent to initialize
sleep 2

# Start neko (this runs in foreground)
echo "Starting neko..."
exec /usr/bin/supervisord -c /etc/supervisord.conf
