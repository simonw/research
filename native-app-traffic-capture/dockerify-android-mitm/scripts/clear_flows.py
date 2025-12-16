#!/usr/bin/env python3
"""
Script to clear all flows from mitmproxy via its API.
This can be run inside the container or from the host.
"""

import sys
import os
import requests
import subprocess
import time
import signal

# Configuration
MITMPROXY_WEB_PORT = int(os.environ.get('MITMPROXY_WEB_PORT', '8081'))
MITMPROXY_PASSWORD = os.environ.get('MITMPROXY_PASSWORD', 'mitmproxy')
MITMPROXY_HOST = os.environ.get('MITMPROXY_HOST', 'localhost')
CONTAINER_NAME = os.environ.get('CONTAINER_NAME', 'dockerify-android-mitm')

def clear_flows_via_api():
    """Try to clear flows via mitmproxy API"""
    url = f"http://{MITMPROXY_HOST}:{MITMPROXY_WEB_PORT}/flows"
    
    try:
        # mitmproxy web UI uses basic auth
        response = requests.delete(
            url,
            auth=('mitmproxy', MITMPROXY_PASSWORD),
            timeout=5
        )
        
        if response.status_code == 200:
            print("✓ Flows cleared via API")
            return True
        else:
            print(f"⚠️  API returned status code: {response.status_code}")
            return False
    except requests.exceptions.RequestException as e:
        print(f"⚠️  API call failed: {e}")
        return False

def restart_mitmproxy():
    """Restart mitmproxy to clear flows"""
    print("Restarting mitmproxy to clear flows...")
    
    # Find mitmproxy process
    try:
        result = subprocess.run(
            ["pgrep", "-f", "mitmweb.*--web-port.*8081"],
            capture_output=True,
            text=True,
            check=False
        )
        
        if result.returncode == 0 and result.stdout.strip():
            pid = int(result.stdout.strip().split('\n')[0])
            print(f"Found mitmproxy process (PID: {pid})")
            
            # Kill the process
            try:
                os.kill(pid, signal.SIGTERM)
                time.sleep(2)
                
                # Check if still running
                try:
                    os.kill(pid, 0)
                    # Still running, force kill
                    os.kill(pid, signal.SIGKILL)
                    time.sleep(1)
                except ProcessLookupError:
                    pass
                
            except ProcessLookupError:
                print("Process already terminated")
        else:
            print("⚠️  mitmproxy process not found")
    except Exception as e:
        print(f"⚠️  Error finding process: {e}")
    
    # Restart mitmproxy
    mitmweb_cmd = [
        "mitmweb",
        "--web-host", "0.0.0.0",
        "--web-port", "8081",
        "--listen-host", "0.0.0.0",
        "--listen-port", "8080",
        "--ssl-insecure",
        "--set", "block_global=false",
        "--set", f"web_password={MITMPROXY_PASSWORD}",
        "--no-web-open-browser"
    ]
    
    try:
        with open("/var/log/mitmproxy.log", "a") as log_file:
            subprocess.Popen(
                mitmweb_cmd,
                stdout=log_file,
                stderr=subprocess.STDOUT,
                start_new_session=True
            )
        print("✓ mitmproxy restarted (flows cleared)")
        return True
    except Exception as e:
        print(f"⚠️  Failed to restart mitmproxy: {e}")
        return False

def main():
    print("=" * 50)
    print(" Clearing mitmproxy flows")
    print("=" * 50)
    
    # Try API first
    if clear_flows_via_api():
        print("=" * 50)
        print("✓ Done!")
        print("=" * 50)
        return 0
    
    # Fallback to restart
    print("\nFalling back to restart method...")
    if restart_mitmproxy():
        print("=" * 50)
        print("✓ Done!")
        print("=" * 50)
        return 0
    else:
        print("=" * 50)
        print("⚠️  Failed to clear flows")
        print("=" * 50)
        return 1

if __name__ == "__main__":
    sys.exit(main())

