"""
just_bash_py - Python client for the just-bash JSONL server.

Provides both sync and async APIs for executing bash commands in a
sandboxed virtual environment powered by just-bash (via Deno).

Usage (sync):
    from just_bash_py import JustBash

    with JustBash() as bash:
        result = bash.run("echo hello")
        print(result.stdout)  # "hello\n"

Usage (async):
    from just_bash_py import AsyncJustBash

    async with AsyncJustBash() as bash:
        result = await bash.run("echo hello")
        print(result.stdout)
"""

from .client import JustBash, AsyncJustBash, BashResult

__all__ = ["JustBash", "AsyncJustBash", "BashResult"]
