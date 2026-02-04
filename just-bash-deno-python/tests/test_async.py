"""Tests for the asynchronous AsyncJustBash client."""

import pytest

from just_bash_py import AsyncJustBash, BashResult


@pytest.fixture(scope="module")
async def bash():
    """Shared AsyncJustBash instance for all tests in this module."""
    async with AsyncJustBash(network=True) as b:
        yield b


@pytest.mark.asyncio(loop_scope="module")
class TestAsyncPing:
    async def test_ping(self, bash):
        assert await bash.ping() is True


@pytest.mark.asyncio(loop_scope="module")
class TestAsyncBasicCommands:
    async def test_echo(self, bash):
        result = await bash.run("echo hello")
        assert result.stdout.strip() == "hello"
        assert result.exit_code == 0

    async def test_arithmetic(self, bash):
        result = await bash.run("echo $((2 ** 10))")
        assert result.stdout.strip() == "1024"

    async def test_printf(self, bash):
        result = await bash.run('printf "%05d\\n" 42')
        assert result.stdout == "00042\n"


@pytest.mark.asyncio(loop_scope="module")
class TestAsyncPipelines:
    async def test_sort(self, bash):
        result = await bash.run('echo -e "z\\na\\nm" | sort')
        assert result.stdout.strip() == "a\nm\nz"

    async def test_jq(self, bash):
        result = await bash.run('echo \'[1,2,3]\' | jq ".[1]"')
        assert result.stdout.strip() == "2"

    async def test_awk(self, bash):
        result = await bash.run(
            'echo -e "alice 90\\nbob 85\\ncarol 95" | awk \'$2 > 89 {print $1}\''
        )
        assert "alice" in result.stdout
        assert "carol" in result.stdout
        assert "bob" not in result.stdout


@pytest.mark.asyncio(loop_scope="module")
class TestAsyncFilesystem:
    async def test_write_and_read(self, bash):
        await bash.run('echo "async test data" > /tmp/async_test.txt')
        result = await bash.run("cat /tmp/async_test.txt")
        assert result.stdout.strip() == "async test data"

    async def test_persistence(self, bash):
        await bash.run("mkdir -p /async_workspace")
        await bash.run('echo "step1" > /async_workspace/log.txt')
        await bash.run('echo "step2" >> /async_workspace/log.txt')

        result = await bash.run("cat /async_workspace/log.txt")
        assert "step1" in result.stdout
        assert "step2" in result.stdout

    async def test_write_file_helper(self, bash):
        await bash.write_file("/tmp/async_helper.txt", "async helper content")
        content = await bash.read_file("/tmp/async_helper.txt")
        assert content.strip() == "async helper content"

    async def test_read_nonexistent(self, bash):
        with pytest.raises(FileNotFoundError):
            await bash.read_file("/no/such/file")


@pytest.mark.asyncio(loop_scope="module")
class TestAsyncScripts:
    async def test_loop(self, bash):
        result = await bash.run(
            'for fruit in apple banana cherry; do echo "$fruit"; done'
        )
        lines = result.stdout.strip().split("\n")
        assert lines == ["apple", "banana", "cherry"]

    async def test_while_loop(self, bash):
        result = await bash.run("i=0; while [ $i -lt 5 ]; do echo $i; i=$((i+1)); done")
        lines = result.stdout.strip().split("\n")
        assert lines == ["0", "1", "2", "3", "4"]


@pytest.mark.asyncio(loop_scope="module")
class TestAsyncErrorHandling:
    async def test_command_failure(self, bash):
        result = await bash.run("false")
        assert result.exit_code != 0
        assert result.ok is False

    async def test_stderr(self, bash):
        result = await bash.run("cat /nonexistent_async 2>&1")
        assert result.exit_code != 0


@pytest.mark.asyncio(loop_scope="module")
class TestAsyncEnvOverride:
    async def test_env(self, bash):
        result = await bash.run('echo "$ASYNC_VAR"', env={"ASYNC_VAR": "async_value"})
        assert result.stdout.strip() == "async_value"


@pytest.mark.asyncio(loop_scope="module")
class TestAsyncCurl:
    async def test_curl_get(self, bash):
        result = await bash.run("curl -s https://httpbin.org/get | jq -r .url")
        assert result.stdout.strip() == "https://httpbin.org/get"


@pytest.mark.asyncio(loop_scope="module")
class TestAsyncReset:
    async def test_reset(self, bash):
        await bash.run('echo "pre-reset" > /tmp/async_reset.txt')
        result = await bash.run("cat /tmp/async_reset.txt")
        assert result.ok

        await bash.reset()

        result = await bash.run("cat /tmp/async_reset.txt")
        assert not result.ok


@pytest.mark.asyncio
class TestAsyncContextManager:
    async def test_context_manager(self):
        async with AsyncJustBash() as bash:
            result = await bash.run("echo works")
            assert result.stdout.strip() == "works"
        assert bash._process is None

    async def test_not_started_raises(self):
        bash = AsyncJustBash()
        with pytest.raises(RuntimeError, match="Server not started"):
            await bash.run("echo hello")
