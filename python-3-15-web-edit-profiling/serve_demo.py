"""Run the unmodified PyPI app on a temporary loopback port."""

import argparse
import asyncio
import json
import os
import socket
import time
from pathlib import Path

import uvicorn
from web_edit.app import create_app


async def serve(directory: Path, state: Path, timeout: float):
    # Binding port zero avoids port-selection races with other local servers.
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        server = uvicorn.Server(
            uvicorn.Config(
                create_app(directory),
                log_level="warning",
                access_log=False,
                lifespan="off",
            )
        )
        task = asyncio.create_task(server.serve(sockets=[sock]))
        try:
            while not server.started:
                if task.done():
                    await task
                    raise RuntimeError("Server exited before becoming ready")
                await asyncio.sleep(0.02)
            ready = state / "ready.json"
            temporary = state / "ready.tmp"
            temporary.write_text(
                json.dumps({"pid": os.getpid(), "port": sock.getsockname()[1]})
            )
            temporary.replace(ready)
            deadline = time.monotonic() + timeout
            while not (state / "stop").exists() and time.monotonic() < deadline:
                if task.done():
                    await task
                    return
                await asyncio.sleep(0.1)
        finally:
            server.should_exit = True
            await task


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("state", type=Path)
    parser.add_argument("--timeout", type=float, default=120)
    args = parser.parse_args()
    asyncio.run(serve(args.directory, args.state, args.timeout))
