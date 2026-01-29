"""
Test time limit capabilities inside a BoxLite sandbox.

Requires KVM-enabled Linux to run. Install: pip install boxlite

Key finding from code analysis: The Python SDK does NOT currently expose
execution timeouts. The CodeBox.run(timeout=) parameter exists but is
not yet implemented. Time limits must be enforced via:
1. asyncio.wait_for() on the host side
2. RLIMIT_CPU in SecurityOptions (applied to the shim process, not guest)
3. The execution.kill() method for manual cancellation
"""

import asyncio
import boxlite


async def test_asyncio_timeout():
    """Test enforcing time limits via asyncio.wait_for()."""
    print("=== Test: asyncio.wait_for() timeout ===")
    async with boxlite.SimpleBox(image="python:slim") as box:
        try:
            result = await asyncio.wait_for(
                box.exec("python", "-c", "import time; time.sleep(30); print('done')"),
                timeout=5.0,
            )
            print(f"  Completed: {result.stdout}")
        except asyncio.TimeoutError:
            print("  TimeoutError raised after 5s (expected)")
        except Exception as e:
            print(f"  Other error: {type(e).__name__}: {e}")


async def test_long_running_process():
    """Test a long-running process to see default behavior."""
    print("\n=== Test: Long-running process (10s) ===")
    async with boxlite.SimpleBox(image="python:slim") as box:
        import time
        start = time.time()
        result = await box.exec(
            "python", "-c",
            "import time\n"
            "for i in range(10):\n"
            "    print(f'tick {i}', flush=True)\n"
            "    time.sleep(1)\n"
            "print('completed')"
        )
        elapsed = time.time() - start
        print(f"  Elapsed: {elapsed:.1f}s")
        print(f"  Last line: {result.stdout.strip().splitlines()[-1] if result.stdout else '(empty)'}")
        print(f"  Exit code: {result.exit_code}")


async def test_execution_kill():
    """Test killing a running execution."""
    print("\n=== Test: Execution.kill() ===")
    runtime = boxlite.Boxlite.default()
    box_opts = boxlite.BoxOptions(image="python:slim")
    box = await runtime.create(box_opts)
    try:
        await box.__aenter__()

        # Start a long-running command
        execution = await box.exec("python", "-c", "import time; time.sleep(60)")

        # Wait a bit then kill it
        await asyncio.sleep(2)
        await execution.kill(signal=9)

        result = await execution.wait()
        print(f"  Exit code after kill: {result.exit_code}")
        print(f"  (Expected non-zero exit code, e.g. -9 or 137)")
    except Exception as e:
        print(f"  Error: {type(e).__name__}: {e}")
    finally:
        await box.__aexit__(None, None, None)


async def test_cpu_intensive_timeout():
    """Test timeout on CPU-intensive workload (not just sleep)."""
    print("\n=== Test: CPU-intensive with timeout ===")
    async with boxlite.SimpleBox(image="python:slim") as box:
        import time
        start = time.time()
        try:
            result = await asyncio.wait_for(
                box.exec(
                    "python", "-c",
                    "# CPU-intensive: compute primes\n"
                    "n = 0\n"
                    "for i in range(2, 10**8):\n"
                    "    if all(i % j != 0 for j in range(2, int(i**0.5)+1)):\n"
                    "        n += 1\n"
                    "print(f'Found {n} primes')"
                ),
                timeout=5.0,
            )
            print(f"  Completed: {result.stdout}")
        except asyncio.TimeoutError:
            elapsed = time.time() - start
            print(f"  TimeoutError after {elapsed:.1f}s (expected)")
        except Exception as e:
            print(f"  Error: {type(e).__name__}: {e}")


async def test_fork_bomb_protection():
    """Test fork bomb protection (RLIMIT_NPROC / cgroups pids.max)."""
    print("\n=== Test: Fork bomb protection ===")
    async with boxlite.SimpleBox(image="python:slim") as box:
        result = await box.exec(
            "python", "-c",
            "import os, sys\n"
            "pids = []\n"
            "try:\n"
            "    for i in range(200):\n"
            "        pid = os.fork()\n"
            "        if pid == 0:\n"
            "            os._exit(0)\n"
            "        pids.append(pid)\n"
            "    print(f'Forked {len(pids)} children')\n"
            "except OSError as e:\n"
            "    print(f'Fork failed after {len(pids)} children: {e}')\n"
            "for p in pids:\n"
            "    try:\n"
            "        os.waitpid(p, 0)\n"
            "    except: pass"
        )
        print(f"  stdout: {result.stdout}")
        print(f"  exit_code: {result.exit_code}")


async def main():
    await test_asyncio_timeout()
    await test_long_running_process()
    # await test_execution_kill()  # Requires low-level API
    await test_cpu_intensive_timeout()
    await test_fork_bomb_protection()


if __name__ == "__main__":
    asyncio.run(main())
