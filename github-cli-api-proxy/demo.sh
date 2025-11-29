#!/bin/bash

# GitHub CLI Proxy Demo Script
# This script demonstrates different methods to proxy gh traffic

set -e

echo "==================================================="
echo "GitHub CLI Proxy Methods Demonstration"
echo "==================================================="
echo

# Check if gh is installed
if ! command -v gh &> /dev/null; then
    echo "Error: gh (GitHub CLI) is not installed"
    echo "Install from: https://cli.github.com/"
    exit 1
fi

# Check if Go is installed
if ! command -v go &> /dev/null; then
    echo "Error: Go is not installed (needed to run example proxies)"
    echo "Install from: https://golang.org/"
    exit 1
fi

echo "Prerequisites: ✓ gh installed, ✓ Go installed"
echo

# Function to test gh with current settings
test_gh() {
    echo "Testing gh with current configuration..."
    gh api /zen 2>/dev/null || echo "  (Request failed - this is expected if not authenticated)"
}

# Method 1: HTTP/HTTPS Proxy
echo "==================================================="
echo "Method 1: Standard HTTP/HTTPS Proxy"
echo "==================================================="
echo
echo "Setting HTTPS_PROXY environment variable..."
echo "  export HTTPS_PROXY=http://localhost:8888"
echo
echo "To test this:"
echo "  1. In terminal 1: go run simple-http-proxy.go 8888"
echo "  2. In terminal 2: export HTTPS_PROXY=http://localhost:8888"
echo "  3. In terminal 2: gh repo view cli/cli"
echo
echo "The proxy will log all gh requests."
echo
read -p "Press Enter to continue..."

# Method 2: Unix Socket Proxy
echo
echo "==================================================="
echo "Method 2: Unix Domain Socket Proxy"
echo "==================================================="
echo
echo "Configuring gh to use Unix socket..."
echo "  gh config set http_unix_socket /tmp/gh-proxy.sock"
echo
echo "To test this:"
echo "  1. In terminal 1: go run unix-socket-proxy.go /tmp/gh-proxy.sock"
echo "  2. In terminal 2: gh config set http_unix_socket /tmp/gh-proxy.sock"
echo "  3. In terminal 2: gh repo view cli/cli"
echo "  4. To reset: gh config set http_unix_socket \"\""
echo
echo "The proxy will log all gh requests."
echo
read -p "Press Enter to continue..."

# Method 3: GH_HOST
echo
echo "==================================================="
echo "Method 3: GH_HOST (for GitHub Enterprise)"
echo "==================================================="
echo
echo "Setting GH_HOST environment variable..."
echo "  export GH_HOST=github.enterprise.example.com"
echo
echo "Note: This does NOT proxy traffic!"
echo "It changes which GitHub instance gh connects to."
echo
echo "This is useful for GitHub Enterprise Server deployments."
echo "You can combine it with HTTPS_PROXY if needed:"
echo "  export GH_HOST=ghe.company.com"
echo "  export HTTPS_PROXY=http://proxy.company.com:3128"
echo "  gh repo view myorg/myrepo"
echo
read -p "Press Enter to continue..."

# Summary
echo
echo "==================================================="
echo "Summary"
echo "==================================================="
echo
echo "Three methods to proxy GitHub CLI traffic:"
echo
echo "1. HTTP/HTTPS Proxy (Easiest, Recommended)"
echo "   • export HTTPS_PROXY=http://proxy.example.com:8080"
echo "   • Works with any standard HTTP proxy"
echo "   • No gh configuration needed"
echo
echo "2. Unix Socket (Most Flexible)"
echo "   • gh config set http_unix_socket /path/to/socket"
echo "   • Requires custom proxy listening on Unix socket"
echo "   • Best for debugging and custom logic"
echo
echo "3. GH_HOST (Different Use Case)"
echo "   • export GH_HOST=github.enterprise.com"
echo "   • Changes target host (not a proxy!)"
echo "   • For GitHub Enterprise Server"
echo
echo "Example proxy servers included:"
echo "  • simple-http-proxy.go - Standard HTTP/HTTPS proxy"
echo "  • unix-socket-proxy.go - Unix socket proxy"
echo
echo "Both log all requests for debugging."
echo
echo "See README.md for detailed information and examples."
echo
