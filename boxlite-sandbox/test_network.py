"""
Test network request capabilities inside a BoxLite sandbox.

Requires KVM-enabled Linux to run. Install: pip install boxlite
"""

import asyncio
import boxlite


async def test_outbound_http():
    """Test outbound HTTP requests from within the sandbox."""
    print("=== Test: Outbound HTTP requests ===")
    async with boxlite.SimpleBox(image="python:slim") as box:
        # Install requests
        await box.exec("pip", "install", "requests")

        # Try hitting a public URL
        result = await box.exec(
            "python", "-c",
            "import requests\n"
            "try:\n"
            "    r = requests.get('https://httpbin.org/ip', timeout=10)\n"
            "    print(f'status={r.status_code}')\n"
            "    print(f'body={r.text[:200]}')\n"
            "except Exception as e:\n"
            "    print(f'error: {e}')"
        )
        print(f"  HTTP GET: {result.stdout}")
        print(f"  stderr: {result.stderr}")


async def test_dns_resolution():
    """Test DNS resolution from within the sandbox."""
    print("\n=== Test: DNS resolution ===")
    async with boxlite.SimpleBox(image="python:slim") as box:
        result = await box.exec(
            "python", "-c",
            "import socket\n"
            "try:\n"
            "    ip = socket.gethostbyname('github.com')\n"
            "    print(f'github.com -> {ip}')\n"
            "except Exception as e:\n"
            "    print(f'DNS failed: {e}')"
        )
        print(f"  DNS: {result.stdout}")


async def test_outbound_tcp():
    """Test raw TCP connections from the sandbox."""
    print("\n=== Test: Raw TCP connections ===")
    async with boxlite.SimpleBox(image="python:slim") as box:
        result = await box.exec(
            "python", "-c",
            "import socket\n"
            "try:\n"
            "    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)\n"
            "    s.settimeout(5)\n"
            "    s.connect(('1.1.1.1', 53))\n"
            "    print('TCP connect to 1.1.1.1:53 succeeded')\n"
            "    s.close()\n"
            "except Exception as e:\n"
            "    print(f'TCP connect failed: {e}')"
        )
        print(f"  TCP: {result.stdout}")


async def test_port_forwarding():
    """Test port forwarding from host to guest."""
    print("\n=== Test: Port forwarding ===")
    async with boxlite.SimpleBox(
        image="python:slim",
        ports=[(18080, 8080, "tcp")],
    ) as box:
        # Start a simple HTTP server in the guest
        await box.exec(
            "python", "-c",
            "import http.server, threading, time\n"
            "def run():\n"
            "    server = http.server.HTTPServer(('0.0.0.0', 8080), http.server.SimpleHTTPRequestHandler)\n"
            "    server.serve_forever()\n"
            "t = threading.Thread(target=run, daemon=True)\n"
            "t.start()\n"
            "time.sleep(1)\n"
            "print('Server started on :8080')"
        )

        # Try connecting from host (would need separate test)
        print("  Server started. Host should be able to connect to localhost:18080")
        print("  (Port forwarding test requires manual verification from host)")


async def test_network_isolation_between_boxes():
    """Test that two boxes cannot communicate directly."""
    print("\n=== Test: Network isolation between boxes ===")
    async with boxlite.SimpleBox(image="python:slim") as box1:
        async with boxlite.SimpleBox(image="python:slim") as box2:
            # Get IP of box1
            r1 = await box1.exec(
                "python", "-c",
                "import socket\n"
                "print(socket.gethostbyname(socket.gethostname()))"
            )
            box1_ip = r1.stdout.strip()
            print(f"  Box1 IP: {box1_ip}")

            r2 = await box2.exec(
                "python", "-c",
                "import socket\n"
                "print(socket.gethostbyname(socket.gethostname()))"
            )
            box2_ip = r2.stdout.strip()
            print(f"  Box2 IP: {box2_ip}")

            # Try pinging box1 from box2
            result = await box2.exec(
                "python", "-c",
                f"import socket\n"
                f"try:\n"
                f"    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)\n"
                f"    s.settimeout(3)\n"
                f"    s.connect(('{box1_ip}', 8080))\n"
                f"    print('CONNECTED - boxes can communicate!')\n"
                f"    s.close()\n"
                f"except Exception as e:\n"
                f"    print(f'Cannot connect: {{e}} - boxes are isolated')"
            )
            print(f"  Cross-box TCP: {result.stdout}")


async def main():
    await test_outbound_http()
    await test_dns_resolution()
    await test_outbound_tcp()
    await test_port_forwarding()
    await test_network_isolation_between_boxes()


if __name__ == "__main__":
    asyncio.run(main())
