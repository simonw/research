"""
Test file read/write capabilities inside a BoxLite sandbox.

Requires KVM-enabled Linux to run. Install: pip install boxlite
"""

import asyncio
import boxlite


async def test_file_write_inside_sandbox():
    """Test writing files within the guest VM."""
    print("=== Test: File write inside sandbox ===")
    async with boxlite.SimpleBox(image="python:slim") as box:
        # Write a file inside the container
        result = await box.exec(
            "python", "-c",
            "with open('/tmp/test.txt', 'w') as f: f.write('hello sandbox')\n"
            "print('write ok')"
        )
        print(f"  Write result: stdout={result.stdout!r}, exit={result.exit_code}")

        # Read it back
        result = await box.exec("cat", "/tmp/test.txt")
        print(f"  Read back: stdout={result.stdout!r}, exit={result.exit_code}")

        # Try writing to /root
        result = await box.exec(
            "python", "-c",
            "with open('/root/test.txt', 'w') as f: f.write('hello root')\n"
            "print('write ok')"
        )
        print(f"  Write to /root: stdout={result.stdout!r}, exit={result.exit_code}")

        # Try writing to / (should work as root inside container)
        result = await box.exec(
            "python", "-c",
            "try:\n"
            "    with open('/test_root.txt', 'w') as f: f.write('hello')\n"
            "    print('write ok')\n"
            "except Exception as e:\n"
            "    print(f'write failed: {e}')"
        )
        print(f"  Write to /: stdout={result.stdout!r}, exit={result.exit_code}")


async def test_file_read_host_paths():
    """Test reading host filesystem paths from the guest."""
    print("\n=== Test: Read host filesystem from guest ===")
    async with boxlite.SimpleBox(image="python:slim") as box:
        # Try reading /etc/passwd (guest's own)
        result = await box.exec("cat", "/etc/passwd")
        print(f"  /etc/passwd lines: {len(result.stdout.splitlines())}, exit={result.exit_code}")

        # Try reading /proc/1/cmdline (guest PID 1)
        result = await box.exec("cat", "/proc/1/cmdline")
        print(f"  /proc/1/cmdline: {result.stdout!r}, exit={result.exit_code}")

        # Try accessing /dev/kvm from inside the guest (shouldn't exist)
        result = await box.exec("ls", "-la", "/dev/kvm")
        print(f"  /dev/kvm: stdout={result.stdout!r}, stderr={result.stderr!r}, exit={result.exit_code}")


async def test_volume_mounts():
    """Test volume mount read/write behavior."""
    print("\n=== Test: Volume mounts ===")
    import tempfile
    import os

    with tempfile.TemporaryDirectory() as tmpdir:
        # Create a test file on host
        test_file = os.path.join(tmpdir, "host_file.txt")
        with open(test_file, "w") as f:
            f.write("data from host")

        # Mount read-only
        async with boxlite.SimpleBox(
            image="python:slim",
            volumes=[(tmpdir, "/mnt/data", "ro")],
        ) as box:
            result = await box.exec("cat", "/mnt/data/host_file.txt")
            print(f"  Read host file (ro): stdout={result.stdout!r}, exit={result.exit_code}")

            result = await box.exec(
                "python", "-c",
                "try:\n"
                "    with open('/mnt/data/new.txt', 'w') as f: f.write('test')\n"
                "    print('write ok')\n"
                "except Exception as e:\n"
                "    print(f'write failed: {e}')"
            )
            print(f"  Write to ro mount: stdout={result.stdout!r}, exit={result.exit_code}")

        # Mount read-write
        async with boxlite.SimpleBox(
            image="python:slim",
            volumes=[(tmpdir, "/mnt/data", "rw")],
        ) as box:
            result = await box.exec(
                "python", "-c",
                "with open('/mnt/data/guest_file.txt', 'w') as f: f.write('from guest')\n"
                "print('write ok')"
            )
            print(f"  Write to rw mount: stdout={result.stdout!r}, exit={result.exit_code}")

        # Check if file appeared on host
        guest_file = os.path.join(tmpdir, "guest_file.txt")
        if os.path.exists(guest_file):
            with open(guest_file) as f:
                print(f"  Host sees guest file: {f.read()!r}")
        else:
            print("  Guest file NOT visible on host")


async def test_copy_in_out():
    """Test copy_in and copy_out file transfer."""
    print("\n=== Test: copy_in / copy_out ===")
    import tempfile
    import os

    with tempfile.TemporaryDirectory() as tmpdir:
        # Create a file to copy in
        src = os.path.join(tmpdir, "input.txt")
        with open(src, "w") as f:
            f.write("copy this into the sandbox")

        async with boxlite.SimpleBox(image="python:slim") as box:
            await box.copy_in(src, "/root/input.txt")
            result = await box.exec("cat", "/root/input.txt")
            print(f"  copy_in result: stdout={result.stdout!r}, exit={result.exit_code}")

            # Create a file inside and copy out
            await box.exec(
                "python", "-c",
                "with open('/root/output.txt', 'w') as f: f.write('from sandbox')"
            )
            dest = os.path.join(tmpdir, "output.txt")
            await box.copy_out("/root/output.txt", dest)
            if os.path.exists(dest):
                with open(dest) as f:
                    print(f"  copy_out result: {f.read()!r}")
            else:
                print("  copy_out file not found")


async def main():
    await test_file_write_inside_sandbox()
    await test_file_read_host_paths()
    await test_volume_mounts()
    await test_copy_in_out()


if __name__ == "__main__":
    asyncio.run(main())
