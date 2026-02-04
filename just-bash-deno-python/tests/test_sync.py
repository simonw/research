"""Tests for the synchronous JustBash client."""

import pytest

from just_bash_py import JustBash, BashResult


@pytest.fixture(scope="module")
def bash():
    """Shared JustBash instance for all tests in this module."""
    with JustBash(network=True) as b:
        yield b


class TestBashResult:
    def test_ok_true(self):
        r = BashResult(stdout="hi", stderr="", exit_code=0)
        assert r.ok is True

    def test_ok_false(self):
        r = BashResult(stdout="", stderr="err", exit_code=1)
        assert r.ok is False

    def test_frozen(self):
        r = BashResult(stdout="hi", stderr="", exit_code=0)
        with pytest.raises(AttributeError):
            r.stdout = "changed"  # type: ignore


class TestSyncPing:
    def test_ping(self, bash):
        assert bash.ping() is True


class TestSyncBasicCommands:
    def test_echo(self, bash):
        result = bash.run("echo hello")
        assert result.stdout.strip() == "hello"
        assert result.exit_code == 0

    def test_echo_with_variable(self, bash):
        result = bash.run('NAME="World" && echo "Hello, $NAME!"')
        assert result.stdout.strip() == "Hello, World!"

    def test_arithmetic(self, bash):
        result = bash.run("echo $((6 * 7))")
        assert result.stdout.strip() == "42"

    def test_printf(self, bash):
        result = bash.run('printf "x=%d\\n" 42')
        assert result.stdout == "x=42\n"


class TestSyncPipelines:
    def test_sort(self, bash):
        result = bash.run('echo -e "c\\na\\nb" | sort')
        assert result.stdout.strip() == "a\nb\nc"

    def test_grep(self, bash):
        result = bash.run('echo -e "apple\\nbanana\\ncherry" | grep "an"')
        assert result.stdout.strip() == "banana"

    def test_jq(self, bash):
        result = bash.run('echo \'{"key": "value"}\' | jq -r .key')
        assert result.stdout.strip() == "value"

    def test_awk_sum(self, bash):
        result = bash.run("seq 1 10 | awk '{sum+=$1} END{print sum}'")
        assert result.stdout.strip() == "55"

    def test_sed(self, bash):
        result = bash.run('echo "hello world" | sed "s/world/python/"')
        assert result.stdout.strip() == "hello python"

    def test_tr(self, bash):
        result = bash.run('echo "hello" | tr "a-z" "A-Z"')
        assert result.stdout.strip() == "HELLO"

    def test_cut(self, bash):
        result = bash.run('echo "a:b:c" | cut -d: -f2')
        assert result.stdout.strip() == "b"

    def test_wc(self, bash):
        result = bash.run("seq 1 50 | wc -l")
        assert result.stdout.strip() == "50"


class TestSyncFilesystem:
    def test_write_and_read(self, bash):
        bash.run('echo "test data" > /tmp/sync_test.txt')
        result = bash.run("cat /tmp/sync_test.txt")
        assert result.stdout.strip() == "test data"

    def test_persistence_across_commands(self, bash):
        bash.run("mkdir -p /workspace")
        bash.run('echo "line 1" > /workspace/data.txt')
        bash.run('echo "line 2" >> /workspace/data.txt')
        bash.run('echo "line 3" >> /workspace/data.txt')

        result = bash.run("wc -l < /workspace/data.txt")
        assert result.stdout.strip() == "3"

        result = bash.run("cat /workspace/data.txt")
        assert "line 1" in result.stdout
        assert "line 3" in result.stdout

    def test_mkdir_and_find(self, bash):
        bash.run("mkdir -p /tmp/testdir/sub1 /tmp/testdir/sub2")
        bash.run("touch /tmp/testdir/a.txt /tmp/testdir/sub1/b.txt")
        result = bash.run('find /tmp/testdir -name "*.txt"')
        assert "/tmp/testdir/a.txt" in result.stdout
        assert "/tmp/testdir/sub1/b.txt" in result.stdout

    def test_write_file_helper(self, bash):
        bash.write_file("/tmp/helper_test.txt", "hello from helper")
        content = bash.read_file("/tmp/helper_test.txt")
        assert content.strip() == "hello from helper"

    def test_read_nonexistent(self, bash):
        with pytest.raises(FileNotFoundError):
            bash.read_file("/nonexistent/path")


class TestSyncScripts:
    def test_for_loop(self, bash):
        result = bash.run("for i in 1 2 3; do echo $i; done")
        assert result.stdout.strip() == "1\n2\n3"

    def test_function(self, bash):
        result = bash.run('greet() { echo "Hi, $1!"; }; greet Python')
        assert result.stdout.strip() == "Hi, Python!"

    def test_conditional(self, bash):
        result = bash.run('if [ 5 -gt 3 ]; then echo "yes"; else echo "no"; fi')
        assert result.stdout.strip() == "yes"

    def test_command_substitution(self, bash):
        result = bash.run('echo "result: $(echo 42)"')
        assert result.stdout.strip() == "result: 42"

    def test_heredoc(self, bash):
        result = bash.run("cat <<EOF\nhello\nworld\nEOF")
        assert result.stdout.strip() == "hello\nworld"

    def test_brace_expansion(self, bash):
        result = bash.run("echo {a,b,c}")
        assert result.stdout.strip() == "a b c"


class TestSyncErrorHandling:
    def test_nonexistent_file(self, bash):
        result = bash.run("cat /no/such/file")
        assert result.exit_code != 0

    def test_invalid_command(self, bash):
        result = bash.run("nonexistent_command_xyz")
        assert result.exit_code != 0


class TestSyncEnvOverride:
    def test_per_request_env(self, bash):
        result = bash.run('echo "$MY_VAR"', env={"MY_VAR": "custom_value"})
        assert result.stdout.strip() == "custom_value"


class TestSyncCurl:
    def test_curl_get(self, bash):
        result = bash.run("curl -s https://httpbin.org/get | jq -r .url")
        assert result.stdout.strip() == "https://httpbin.org/get"
        assert result.exit_code == 0

    def test_curl_with_headers(self, bash):
        result = bash.run(
            "curl -s -H 'X-Test: hello' https://httpbin.org/headers | jq -r '.headers[\"X-Test\"]'"
        )
        assert result.stdout.strip() == "hello"


class TestSyncReset:
    def test_reset_clears_files(self, bash):
        bash.run('echo "temp" > /tmp/reset_test.txt')
        result = bash.run("cat /tmp/reset_test.txt")
        assert result.ok

        bash.reset()

        result = bash.run("cat /tmp/reset_test.txt")
        assert not result.ok


class TestSyncBase64:
    def test_encode(self, bash):
        result = bash.run('echo -n "hello" | base64')
        assert result.stdout.strip() == "aGVsbG8="

    def test_decode(self, bash):
        result = bash.run('echo "aGVsbG8=" | base64 -d')
        assert result.stdout == "hello"


class TestSyncSha256:
    def test_sha256sum(self, bash):
        result = bash.run('echo -n "hello" | sha256sum')
        assert result.stdout.startswith("2cf24dba5fb0a30e26e83b2ac5b9e29e")


class TestSyncHtmlToMarkdown:
    def test_convert(self, bash):
        result = bash.run(
            'echo "<h1>Title</h1><p>Hello <strong>world</strong></p>" | html-to-markdown'
        )
        assert "# Title" in result.stdout
        assert "**world**" in result.stdout


class TestContextManager:
    def test_context_manager_starts_and_stops(self):
        with JustBash() as bash:
            result = bash.run("echo works")
            assert result.stdout.strip() == "works"
        # After exiting context, process should be stopped
        assert bash._process is None

    def test_not_started_raises(self):
        bash = JustBash()
        with pytest.raises(RuntimeError, match="Server not started"):
            bash.run("echo hello")
