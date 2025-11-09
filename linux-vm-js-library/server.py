#!/usr/bin/env python3
"""
Simple HTTP server with CORS and SharedArrayBuffer headers for v86
"""

import http.server
import socketserver
from functools import partial


class CORSRequestHandler(http.server.SimpleHTTPRequestHandler):
    """HTTP request handler with CORS and security headers for v86"""

    def end_headers(self):
        # Required headers for SharedArrayBuffer (needed by v86)
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        # CORS headers
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', '*')
        super().end_headers()


def run_server(port=8000):
    """Run HTTP server with proper headers"""
    handler = CORSRequestHandler

    with socketserver.TCPServer(("", port), handler) as httpd:
        print(f"Server running at http://localhost:{port}/")
        print("Press Ctrl+C to stop")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nServer stopped")


if __name__ == "__main__":
    run_server()
