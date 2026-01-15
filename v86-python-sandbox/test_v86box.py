#!/usr/bin/env python3
"""Test script for v86box."""

import sys
sys.path.insert(0, "/home/user/research/v86-python-sandbox")

from v86box import V86box, V86boxError

def main():
    print("Testing v86box...")
    print()

    with V86box(boot_timeout=120.0) as box:
        # Test basic command
        print("Running: echo 'Hello from v86!'")
        result = box.exec("echo 'Hello from v86!'")
        print(f"Output: {result}")
        print()

        # Test uname
        print("Running: uname -a")
        result = box.exec("uname -a")
        print(f"Output: {result}")
        print()

        # Test cat /etc/os-release
        print("Running: cat /etc/os-release")
        result = box.exec("cat /etc/os-release")
        print(f"Output:\n{result}")
        print()

        # Test pwd and ls
        print("Running: pwd")
        result = box.exec("pwd")
        print(f"Output: {result}")

        print("Running: ls -la /")
        result = box.exec("ls -la /")
        print(f"Output:\n{result}")
        print()

        # Test status
        print("Getting status...")
        status = box.status()
        print(f"Status: {status}")
        print()

        print("All tests passed!")

if __name__ == "__main__":
    main()
