"""Tests to verify the Deno sandbox is properly locked down."""

import pytest
from denobox import DenoBox, AsyncDenoBox, DenoBoxError


class TestSandboxSync:
    """Test that the sync sandbox is properly restricted."""

    def test_cannot_read_files(self):
        """Verify Deno cannot read files from the filesystem."""
        with DenoBox() as box:
            with pytest.raises(DenoBoxError) as exc_info:
                box.eval('Deno.readTextFileSync("/etc/passwd")')
            # Error should mention read access requirement
            err = str(exc_info.value).lower()
            assert "read" in err and "access" in err

    def test_cannot_write_files(self):
        """Verify Deno cannot write files to the filesystem."""
        with DenoBox() as box:
            with pytest.raises(DenoBoxError) as exc_info:
                box.eval('Deno.writeTextFileSync("/tmp/test.txt", "hello")')
            # Error should mention write access requirement
            err = str(exc_info.value).lower()
            assert "write" in err and "access" in err

    def test_cannot_make_network_requests(self):
        """Verify Deno cannot make network requests."""
        with DenoBox() as box:
            with pytest.raises(DenoBoxError) as exc_info:
                # Use sync XMLHttpRequest pattern or Promise-based
                box.eval('(async () => await fetch("https://example.com"))()')
            # Error should mention net access requirement
            err = str(exc_info.value).lower()
            assert "net" in err and "access" in err

    def test_cannot_run_subprocess(self):
        """Verify Deno cannot spawn subprocesses."""
        with DenoBox() as box:
            with pytest.raises(DenoBoxError) as exc_info:
                box.eval('new Deno.Command("ls").outputSync()')
            # Error should mention run access requirement
            err = str(exc_info.value).lower()
            assert "run" in err and "access" in err

    def test_cannot_access_env(self):
        """Verify Deno cannot access environment variables."""
        with DenoBox() as box:
            with pytest.raises(DenoBoxError) as exc_info:
                box.eval('Deno.env.get("PATH")')
            # Error should mention env access requirement
            err = str(exc_info.value).lower()
            assert "env" in err and "access" in err

    def test_pure_computation_works(self):
        """Verify pure JavaScript computation still works."""
        with DenoBox() as box:
            # Math
            assert box.eval("Math.sqrt(16)") == 4
            # String manipulation
            assert box.eval('"hello".toUpperCase()') == "HELLO"
            # Array operations
            assert box.eval("[1,2,3].reduce((a,b) => a + b, 0)") == 6
            # JSON
            assert box.eval("JSON.parse('{\"a\": 1}')") == {"a": 1}


class TestSandboxAsync:
    """Test that the async sandbox is properly restricted."""

    async def test_cannot_read_files(self):
        """Verify Deno cannot read files from the filesystem."""
        async with AsyncDenoBox() as box:
            with pytest.raises(DenoBoxError) as exc_info:
                # Use an async IIFE since await isn't top-level in eval
                await box.eval('(async () => await Deno.readTextFile("/etc/passwd"))()')
            err = str(exc_info.value).lower()
            assert "read" in err and "access" in err

    async def test_cannot_write_files(self):
        """Verify Deno cannot write files to the filesystem."""
        async with AsyncDenoBox() as box:
            with pytest.raises(DenoBoxError) as exc_info:
                await box.eval(
                    '(async () => await Deno.writeTextFile("/tmp/test.txt", "hello"))()'
                )
            err = str(exc_info.value).lower()
            assert "write" in err and "access" in err

    async def test_cannot_make_network_requests(self):
        """Verify Deno cannot make network requests."""
        async with AsyncDenoBox() as box:
            with pytest.raises(DenoBoxError) as exc_info:
                await box.eval('(async () => await fetch("https://example.com"))()')
            err = str(exc_info.value).lower()
            assert "net" in err and "access" in err

    async def test_pure_computation_works(self):
        """Verify pure JavaScript computation still works."""
        async with AsyncDenoBox() as box:
            assert await box.eval("Math.PI") == pytest.approx(3.14159, rel=1e-4)
            assert await box.eval("Array.from({length: 5}, (_, i) => i)") == [
                0,
                1,
                2,
                3,
                4,
            ]
