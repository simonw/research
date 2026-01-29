"""
Test memory limits inside a BoxLite sandbox.

Requires KVM-enabled Linux to run. Install: pip install boxlite
"""

import asyncio
import boxlite


async def test_memory_limit_default():
    """Test default memory allocation."""
    print("=== Test: Default memory ===")
    async with boxlite.SimpleBox(image="python:slim") as box:
        result = await box.exec(
            "python", "-c",
            "import os\n"
            "mem = os.sysconf('SC_PAGE_SIZE') * os.sysconf('SC_PHYS_PAGES')\n"
            "print(f'Total memory: {mem / (1024*1024):.0f} MiB')"
        )
        print(f"  {result.stdout.strip()}")


async def test_memory_limit_explicit():
    """Test explicit memory limit (256 MiB)."""
    print("\n=== Test: Explicit memory limit (256 MiB) ===")
    async with boxlite.SimpleBox(image="python:slim", memory_mib=256) as box:
        result = await box.exec(
            "python", "-c",
            "import os\n"
            "mem = os.sysconf('SC_PAGE_SIZE') * os.sysconf('SC_PHYS_PAGES')\n"
            "print(f'Total memory: {mem / (1024*1024):.0f} MiB')"
        )
        print(f"  {result.stdout.strip()}")


async def test_memory_limit_small():
    """Test minimum memory (128 MiB)."""
    print("\n=== Test: Minimum memory (128 MiB) ===")
    try:
        async with boxlite.SimpleBox(image="python:slim", memory_mib=128) as box:
            result = await box.exec(
                "python", "-c",
                "import os\n"
                "mem = os.sysconf('SC_PAGE_SIZE') * os.sysconf('SC_PHYS_PAGES')\n"
                "print(f'Total memory: {mem / (1024*1024):.0f} MiB')"
            )
            print(f"  {result.stdout.strip()}")
    except Exception as e:
        print(f"  Error with 128 MiB: {e}")


async def test_memory_exhaustion():
    """Test what happens when the guest exhausts its memory."""
    print("\n=== Test: Memory exhaustion (allocate more than limit) ===")
    async with boxlite.SimpleBox(image="python:slim", memory_mib=256) as box:
        result = await box.exec(
            "python", "-c",
            "import sys\n"
            "blocks = []\n"
            "try:\n"
            "    for i in range(1000):\n"
            "        blocks.append(b'x' * (1024 * 1024))  # 1 MiB per block\n"
            "        if (i+1) % 50 == 0:\n"
            "            print(f'Allocated {i+1} MiB', flush=True)\n"
            "except MemoryError:\n"
            "    print(f'MemoryError after {len(blocks)} MiB')\n"
            "except Exception as e:\n"
            "    print(f'Other error: {type(e).__name__}: {e}')"
        )
        print(f"  stdout: {result.stdout}")
        print(f"  stderr: {result.stderr[:200] if result.stderr else '(none)'}")
        print(f"  exit_code: {result.exit_code}")


async def test_memory_metrics():
    """Test memory metrics reporting."""
    print("\n=== Test: Memory metrics ===")
    runtime = boxlite.Boxlite.default()
    box_opts = boxlite.BoxOptions(image="python:slim", memory_mib=512)
    box = await runtime.create(box_opts)
    try:
        await box.__aenter__()

        # Allocate some memory
        await box.exec(
            "python", "-c",
            "x = b'a' * (100 * 1024 * 1024)\nprint('allocated 100 MiB')"
        )

        # Check metrics
        metrics = await box.metrics()
        print(f"  Memory usage: {metrics.memory_usage_bytes / (1024*1024):.1f} MiB")
        print(f"  CPU time: {metrics.cpu_time_ms} ms")
    finally:
        await box.__aexit__(None, None, None)


async def main():
    await test_memory_limit_default()
    await test_memory_limit_explicit()
    await test_memory_limit_small()
    await test_memory_exhaustion()
    # await test_memory_metrics()  # Requires low-level API


if __name__ == "__main__":
    asyncio.run(main())
