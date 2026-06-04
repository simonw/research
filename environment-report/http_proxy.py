#!/usr/bin/env python3
"""
HTTP Proxy that listens on a Unix socket and forwards requests to the internet.

This allows Docker containers running with --network=none to make HTTP requests
by connecting to this proxy via a mounted Unix socket.

Usage:
    # Start the proxy on the host
    python3 http_proxy.py &

    # Run a container with the socket mounted
    docker run --network=none -v /tmp/http_proxy.sock:/tmp/http_proxy.sock myimage

Protocol:
    Send JSON: {"url": "https://example.com"}
    Receive JSON: {"status": 200, "body": "..."} or {"error": "..."}
"""
import socket
import os
import threading
import urllib.request
import json
import sys

SOCKET_PATH = '/tmp/http_proxy.sock'


def handle_client(conn):
    """Handle a single client connection."""
    try:
        # Receive request (simple protocol: JSON with url)
        data = conn.recv(4096).decode()
        request = json.loads(data)
        url = request.get('url')
        method = request.get('method', 'GET')
        headers = request.get('headers', {})
        body = request.get('body')

        print(f"PROXY: {method} {url}")

        # Build request
        req = urllib.request.Request(url, method=method)
        for key, value in headers.items():
            req.add_header(key, value)

        if body:
            req.data = body.encode() if isinstance(body, str) else body

        # Fetch from real network
        try:
            with urllib.request.urlopen(req, timeout=30) as response:
                response_body = response.read()
                result = {
                    'status': response.status,
                    'headers': dict(response.headers),
                    'body': response_body.decode('utf-8', errors='replace')
                }
        except urllib.error.HTTPError as e:
            result = {
                'status': e.code,
                'headers': dict(e.headers),
                'body': e.read().decode('utf-8', errors='replace')
            }
        except Exception as e:
            result = {'error': str(e)}

        conn.send(json.dumps(result).encode())
    except Exception as e:
        print(f"PROXY error: {e}")
        try:
            conn.send(json.dumps({'error': str(e)}).encode())
        except:
            pass
    finally:
        conn.close()


def main():
    """Start the proxy server."""
    if os.path.exists(SOCKET_PATH):
        os.remove(SOCKET_PATH)

    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.bind(SOCKET_PATH)
    os.chmod(SOCKET_PATH, 0o777)
    sock.listen(5)

    print(f"HTTP Proxy listening on {SOCKET_PATH}")
    print("Mount this socket into containers with: -v /tmp/http_proxy.sock:/tmp/http_proxy.sock")

    try:
        while True:
            conn, _ = sock.accept()
            threading.Thread(target=handle_client, args=(conn,), daemon=True).start()
    except KeyboardInterrupt:
        print("\nShutting down...")
    finally:
        sock.close()
        if os.path.exists(SOCKET_PATH):
            os.remove(SOCKET_PATH)


if __name__ == '__main__':
    main()
