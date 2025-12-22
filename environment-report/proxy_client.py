#!/usr/bin/env python3
"""
Client library for making HTTP requests through the Unix socket proxy.

Use this inside Docker containers that don't have network access.

Usage:
    from proxy_client import fetch

    # Simple GET request
    response = fetch('https://api.example.com/data')
    print(response['status'], response['body'])

    # POST request with JSON
    response = fetch('https://api.example.com/data',
                     method='POST',
                     headers={'Content-Type': 'application/json'},
                     body='{"key": "value"}')
"""
import socket
import json

SOCKET_PATH = '/tmp/http_proxy.sock'


def fetch(url, method='GET', headers=None, body=None, socket_path=SOCKET_PATH):
    """
    Make an HTTP request through the Unix socket proxy.

    Args:
        url: The URL to fetch
        method: HTTP method (GET, POST, PUT, DELETE, etc.)
        headers: Optional dict of headers
        body: Optional request body (string or bytes)
        socket_path: Path to the proxy socket

    Returns:
        dict with 'status', 'headers', 'body' on success
        dict with 'error' on failure
    """
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        sock.connect(socket_path)

        request = {
            'url': url,
            'method': method,
        }
        if headers:
            request['headers'] = headers
        if body:
            request['body'] = body

        sock.send(json.dumps(request).encode())

        # Receive response (may need multiple reads for large responses)
        chunks = []
        while True:
            chunk = sock.recv(8192)
            if not chunk:
                break
            chunks.append(chunk)

        response_data = b''.join(chunks).decode()
        return json.loads(response_data)

    finally:
        sock.close()


# Example usage
if __name__ == '__main__':
    import sys

    url = sys.argv[1] if len(sys.argv) > 1 else 'https://httpbin.org/get'
    print(f"Fetching {url} via proxy...")

    result = fetch(url)

    if 'error' in result:
        print(f"Error: {result['error']}")
    else:
        print(f"Status: {result['status']}")
        print(f"Body preview: {result['body'][:500]}")
