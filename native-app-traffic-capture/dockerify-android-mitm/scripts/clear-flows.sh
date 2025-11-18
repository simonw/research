#!/bin/bash
# Script to clear all flows from mitmproxy
# This can be run inside the container or from the host
#
# Usage:
#   From host: ./clear-flows.sh
#   Inside container: ./clear-flows.sh
#
# Environment variables:
#   CONTAINER_NAME - Docker container name (default: dockerify-android-mitm)
#   MITMPROXY_PASSWORD - mitmproxy password (default: mitmproxy)

set -e

# Configuration
MITMPROXY_PASSWORD=${MITMPROXY_PASSWORD:-mitmproxy}
CONTAINER_NAME=${CONTAINER_NAME:-dockerify-android-mitm}

# Check if running inside container or from host
if [ -f /.dockerenv ] || [ -n "$DOCKER_CONTAINER" ]; then
    CONTAINER_MODE=true
else
    CONTAINER_MODE=false
fi

echo "=========================================="
echo " Clearing mitmproxy flows"
echo "=========================================="

clear_flows_inside_container() {
    # Get mitmproxy process PID
    MITMPROXY_PID=$(pgrep -f 'mitmweb.*--web-port.*8081' | head -1)
    
    if [ -z "$MITMPROXY_PID" ]; then
        echo "⚠️  mitmproxy process not found"
        return 1
    fi
    
    echo "Found mitmproxy process (PID: $MITMPROXY_PID)"
    
    # Restart mitmproxy to clear all flows
    echo "Restarting mitmproxy to clear flows..."
    
    # Kill the process gracefully
    kill -TERM "$MITMPROXY_PID" 2>/dev/null || true
    sleep 2
    
    # Force kill if still running
    if kill -0 "$MITMPROXY_PID" 2>/dev/null; then
        kill -KILL "$MITMPROXY_PID" 2>/dev/null || true
        sleep 1
    fi
    
    # Restart mitmproxy with same configuration as entrypoint.sh
    nohup mitmweb \
        --web-host 0.0.0.0 \
        --web-port 8081 \
        --listen-host 0.0.0.0 \
        --listen-port 8080 \
        --ssl-insecure \
        --set block_global=false \
        --set web_password="${MITMPROXY_PASSWORD}" \
        --no-web-open-browser \
        > /var/log/mitmproxy.log 2>&1 &
    
    sleep 2
    
    # Verify it's running
    NEW_PID=$(pgrep -f 'mitmweb.*--web-port.*8081' | head -1)
    if [ -n "$NEW_PID" ]; then
        echo "✓ mitmproxy restarted successfully (PID: $NEW_PID)"
        echo "✓ All flows cleared"
        return 0
    else
        echo "⚠️  Failed to restart mitmproxy"
        return 1
    fi
}

if [ "$CONTAINER_MODE" = false ]; then
    echo "Running from host, executing inside container: $CONTAINER_NAME"
    
    # Check if container exists
    if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        echo "⚠️  Container '$CONTAINER_NAME' is not running"
        echo "Available containers:"
        docker ps --format '  {{.Names}}'
        exit 1
    fi
    
    # Execute inside container - inline the function logic
    docker exec "$CONTAINER_NAME" bash -c "
        MITMPROXY_PID=\$(pgrep -f 'mitmweb.*--web-port.*8081' | head -1)
        
        if [ -z \"\$MITMPROXY_PID\" ]; then
            echo '⚠️  mitmproxy process not found'
            exit 1
        fi
        
        echo \"Found mitmproxy process (PID: \$MITMPROXY_PID)\"
        echo 'Restarting mitmproxy to clear flows...'
        
        kill -TERM \"\$MITMPROXY_PID\" 2>/dev/null || true
        sleep 2
        
        if kill -0 \"\$MITMPROXY_PID\" 2>/dev/null; then
            kill -KILL \"\$MITMPROXY_PID\" 2>/dev/null || true
            sleep 1
        fi
        
        nohup mitmweb \\
            --web-host 0.0.0.0 \\
            --web-port 8081 \\
            --listen-host 0.0.0.0 \\
            --listen-port 8080 \\
            --ssl-insecure \\
            --set block_global=false \\
            --set web_password='${MITMPROXY_PASSWORD}' \\
            --no-web-open-browser \\
            > /var/log/mitmproxy.log 2>&1 &
        
        sleep 2
        
        NEW_PID=\$(pgrep -f 'mitmweb.*--web-port.*8081' | head -1)
        if [ -n \"\$NEW_PID\" ]; then
            echo \"✓ mitmproxy restarted successfully (PID: \$NEW_PID)\"
            echo '✓ All flows cleared'
        else
            echo '⚠️  Failed to restart mitmproxy'
            exit 1
        fi
    "
else
    # Running inside container
    echo "Running inside container"
    clear_flows_inside_container
fi

echo "=========================================="
echo "✓ Done!"
echo "=========================================="

