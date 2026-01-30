"""Tests for the eryx Python bindings.

These tests exercise the Python binding layer built from the eryx Rust library.
They cover:
- Sandbox creation and basic execution
- ExecuteResult fields (stdout, stderr, duration, stats)
- ResourceLimits configuration
- NetConfig configuration
- Session state persistence
- VfsStorage (virtual filesystem)
- Error handling and exception types

Note: The eryx sandbox strips trailing newlines from stdout, so
print("hello") produces stdout == "hello" (not "hello\n").
"""

import pytest

import eryx


# ---------------------------------------------------------------------------
# Module-level checks
# ---------------------------------------------------------------------------

class TestModuleImport:
    """Verify the module imports and exposes expected symbols."""

    def test_version_exists(self):
        assert hasattr(eryx, "__version__")
        assert isinstance(eryx.__version__, str)
        assert len(eryx.__version__) > 0

    def test_public_classes_exist(self):
        assert hasattr(eryx, "Sandbox")
        assert hasattr(eryx, "Session")
        assert hasattr(eryx, "SandboxFactory")
        assert hasattr(eryx, "ExecuteResult")
        assert hasattr(eryx, "ResourceLimits")
        assert hasattr(eryx, "NetConfig")
        assert hasattr(eryx, "VfsStorage")

    def test_exception_classes_exist(self):
        assert hasattr(eryx, "EryxError")
        assert hasattr(eryx, "ExecutionError")
        assert hasattr(eryx, "InitializationError")
        assert hasattr(eryx, "ResourceLimitError")
        assert hasattr(eryx, "TimeoutError")

    def test_eryx_error_is_exception(self):
        assert issubclass(eryx.EryxError, Exception)

    def test_execution_error_inherits_eryx_error(self):
        assert issubclass(eryx.ExecutionError, eryx.EryxError)

    def test_initialization_error_inherits_eryx_error(self):
        assert issubclass(eryx.InitializationError, eryx.EryxError)

    def test_resource_limit_error_inherits_eryx_error(self):
        assert issubclass(eryx.ResourceLimitError, eryx.EryxError)


# ---------------------------------------------------------------------------
# Sandbox basics
# ---------------------------------------------------------------------------

class TestSandbox:
    """Test basic Sandbox creation and execution."""

    def test_create_sandbox(self):
        sandbox = eryx.Sandbox()
        assert sandbox is not None

    def test_execute_hello_world(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('print("Hello from sandbox!")')
        assert result.stdout == "Hello from sandbox!"

    def test_execute_returns_execute_result(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('print("test")')
        assert isinstance(result, eryx.ExecuteResult)

    def test_execute_result_stdout(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('print("abc")')
        assert result.stdout == "abc"

    def test_execute_result_duration(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('x = 1')
        assert isinstance(result.duration_ms, float)
        assert result.duration_ms >= 0

    def test_execute_result_callback_invocations(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('x = 1')
        assert isinstance(result.callback_invocations, int)
        assert result.callback_invocations == 0

    def test_execute_multiline(self):
        sandbox = eryx.Sandbox()
        code = """
x = 2
y = 3
print(x + y)
"""
        result = sandbox.execute(code)
        assert result.stdout == "5"

    def test_execute_stdlib_json(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('import json; print(json.dumps({"a": 1}))')
        assert '"a": 1' in result.stdout or '"a":1' in result.stdout

    def test_execute_stdlib_math(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('import math; print(math.pi)')
        assert result.stdout.startswith("3.14159")

    def test_execute_error_raises_execution_error(self):
        sandbox = eryx.Sandbox()
        with pytest.raises(eryx.ExecutionError):
            sandbox.execute('raise ValueError("test error")')

    def test_execute_syntax_error(self):
        sandbox = eryx.Sandbox()
        with pytest.raises(eryx.ExecutionError):
            sandbox.execute('def invalid syntax')

    def test_execute_name_error(self):
        sandbox = eryx.Sandbox()
        with pytest.raises(eryx.ExecutionError):
            sandbox.execute('print(undefined_variable)')

    def test_sandbox_isolation(self):
        """Each sandbox.execute() call is isolated."""
        sandbox = eryx.Sandbox()
        sandbox.execute('x = 42')
        with pytest.raises(eryx.ExecutionError):
            sandbox.execute('print(x)')

    def test_sandbox_repr(self):
        sandbox = eryx.Sandbox()
        r = repr(sandbox)
        assert "Sandbox" in r

    def test_execute_result_repr(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('print("hi")')
        r = repr(result)
        assert "ExecuteResult" in r

    def test_execute_result_str(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('print("hi")')
        assert str(result) == "hi"

    def test_execute_stderr(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('import sys; print("err", file=sys.stderr)')
        assert "err" in result.stderr


# ---------------------------------------------------------------------------
# ResourceLimits
# ---------------------------------------------------------------------------

class TestResourceLimits:
    """Test ResourceLimits configuration."""

    def test_create_default(self):
        limits = eryx.ResourceLimits()
        assert limits.execution_timeout_ms == 30000
        assert limits.callback_timeout_ms == 10000
        assert limits.max_memory_bytes == 134217728  # 128 MB
        assert limits.max_callback_invocations == 1000

    def test_create_custom(self):
        limits = eryx.ResourceLimits(
            execution_timeout_ms=5000,
            max_memory_bytes=50_000_000,
        )
        assert limits.execution_timeout_ms == 5000
        assert limits.max_memory_bytes == 50_000_000

    def test_unlimited(self):
        limits = eryx.ResourceLimits.unlimited()
        assert limits.execution_timeout_ms is None
        assert limits.callback_timeout_ms is None
        assert limits.max_memory_bytes is None
        assert limits.max_callback_invocations is None

    def test_setters(self):
        limits = eryx.ResourceLimits()
        limits.execution_timeout_ms = 1000
        assert limits.execution_timeout_ms == 1000
        limits.max_memory_bytes = 64_000_000
        assert limits.max_memory_bytes == 64_000_000

    def test_repr(self):
        limits = eryx.ResourceLimits()
        r = repr(limits)
        assert "ResourceLimits" in r
        assert "30000" in r

    def test_sandbox_with_limits(self):
        limits = eryx.ResourceLimits(execution_timeout_ms=60000)
        sandbox = eryx.Sandbox(resource_limits=limits)
        result = sandbox.execute('print("with limits")')
        assert result.stdout == "with limits"

    def test_timeout_raises(self):
        limits = eryx.ResourceLimits(execution_timeout_ms=100)
        sandbox = eryx.Sandbox(resource_limits=limits)
        with pytest.raises((eryx.TimeoutError, eryx.ExecutionError)):
            sandbox.execute("""
import time
while True:
    pass
""")


# ---------------------------------------------------------------------------
# NetConfig
# ---------------------------------------------------------------------------

class TestNetConfig:
    """Test NetConfig configuration."""

    def test_create_default(self):
        net = eryx.NetConfig()
        assert net.max_connections == 10
        assert net.connect_timeout_ms == 30000
        assert net.io_timeout_ms == 60000
        assert net.allowed_hosts == []
        assert len(net.blocked_hosts) > 0  # defaults block localhost etc.

    def test_create_with_allowed_hosts(self):
        net = eryx.NetConfig(allowed_hosts=["api.example.com"])
        assert "api.example.com" in net.allowed_hosts

    def test_permissive(self):
        net = eryx.NetConfig.permissive()
        assert net.max_connections == 100
        assert net.blocked_hosts == []

    def test_allow_host_chaining(self):
        net = eryx.NetConfig()
        result = net.allow_host("example.com")
        assert isinstance(result, eryx.NetConfig)
        assert "example.com" in result.allowed_hosts

    def test_block_host_chaining(self):
        net = eryx.NetConfig()
        result = net.block_host("evil.com")
        assert isinstance(result, eryx.NetConfig)
        assert "evil.com" in result.blocked_hosts

    def test_allow_localhost(self):
        net = eryx.NetConfig()
        initial_blocked = len(net.blocked_hosts)
        result = net.allow_localhost()
        # Should have removed localhost-related entries
        assert len(result.blocked_hosts) < initial_blocked

    def test_repr(self):
        net = eryx.NetConfig()
        r = repr(net)
        assert "NetConfig" in r

    def test_setters(self):
        net = eryx.NetConfig()
        net.max_connections = 5
        assert net.max_connections == 5
        net.connect_timeout_ms = 1000
        assert net.connect_timeout_ms == 1000

    def test_sandbox_with_network(self):
        net = eryx.NetConfig(allowed_hosts=["httpbin.org"])
        sandbox = eryx.Sandbox(network=net)
        result = sandbox.execute('print("with network")')
        assert result.stdout == "with network"


# ---------------------------------------------------------------------------
# Session
# ---------------------------------------------------------------------------

class TestSession:
    """Test Session for persistent state across executions."""

    def test_create_session(self):
        session = eryx.Session()
        assert session is not None

    def test_session_state_persists(self):
        session = eryx.Session()
        session.execute('x = 42')
        result = session.execute('print(x)')
        assert result.stdout == "42"

    def test_session_function_persists(self):
        session = eryx.Session()
        session.execute('def add(a, b): return a + b')
        result = session.execute('print(add(3, 4))')
        assert result.stdout == "7"

    def test_session_incremental_state(self):
        session = eryx.Session()
        session.execute('x = 1')
        session.execute('y = 2')
        session.execute('z = x + y')
        result = session.execute('print(z)')
        assert result.stdout == "3"

    def test_session_execution_count(self):
        session = eryx.Session()
        assert session.execution_count == 0
        session.execute('x = 1')
        assert session.execution_count == 1
        session.execute('y = 2')
        assert session.execution_count == 2

    def test_session_reset(self):
        session = eryx.Session()
        session.execute('x = 42')
        session.reset()
        with pytest.raises(eryx.ExecutionError):
            session.execute('print(x)')

    def test_session_clear_state(self):
        session = eryx.Session()
        session.execute('x = 42')
        session.clear_state()
        with pytest.raises(eryx.ExecutionError):
            session.execute('print(x)')

    def test_session_snapshot_and_restore(self):
        session = eryx.Session()
        session.execute('x = 100')
        snapshot = session.snapshot_state()
        assert isinstance(snapshot, bytes)
        assert len(snapshot) > 0

        session.execute('x = 999')
        session.restore_state(snapshot)
        result = session.execute('print(x)')
        assert result.stdout == "100"

    def test_session_repr(self):
        session = eryx.Session()
        r = repr(session)
        assert "Session" in r

    def test_session_timeout(self):
        session = eryx.Session(execution_timeout_ms=5000)
        assert session.execution_timeout_ms == 5000

    def test_session_set_timeout(self):
        session = eryx.Session()
        session.execution_timeout_ms = 10000
        assert session.execution_timeout_ms == 10000

    def test_session_execute_returns_result(self):
        session = eryx.Session()
        result = session.execute('print("from session")')
        assert isinstance(result, eryx.ExecuteResult)
        assert result.stdout == "from session"

    def test_session_error_handling(self):
        session = eryx.Session()
        with pytest.raises(eryx.ExecutionError):
            session.execute('raise RuntimeError("session error")')

    def test_session_stdlib_access(self):
        session = eryx.Session()
        session.execute('import json')
        result = session.execute('print(json.dumps([1, 2, 3]))')
        assert result.stdout == "[1, 2, 3]"


# ---------------------------------------------------------------------------
# VfsStorage
# ---------------------------------------------------------------------------

class TestVfsStorage:
    """Test virtual filesystem storage."""

    def test_create_vfs(self):
        vfs = eryx.VfsStorage()
        assert vfs is not None

    def test_vfs_repr(self):
        vfs = eryx.VfsStorage()
        assert "VfsStorage" in repr(vfs)

    def test_session_with_vfs(self):
        storage = eryx.VfsStorage()
        session = eryx.Session(vfs=storage)
        assert session.vfs is not None
        assert session.vfs_mount_path == "/data"

    def test_session_vfs_custom_mount(self):
        storage = eryx.VfsStorage()
        session = eryx.Session(vfs=storage, vfs_mount_path="/mnt")
        assert session.vfs_mount_path == "/mnt"

    def test_session_vfs_write_and_read(self):
        storage = eryx.VfsStorage()
        session = eryx.Session(vfs=storage)
        session.execute("""
import os
os.makedirs("/data/test", exist_ok=True)
with open("/data/test/hello.txt", "w") as f:
    f.write("hello vfs")
""")
        result = session.execute("""
with open("/data/test/hello.txt") as f:
    print(f.read())
""")
        assert "hello vfs" in result.stdout

    def test_session_no_vfs(self):
        session = eryx.Session()
        assert session.vfs is None
        assert session.vfs_mount_path is None


# ---------------------------------------------------------------------------
# Integration / edge cases
# ---------------------------------------------------------------------------

class TestIntegration:
    """Integration tests covering cross-cutting scenarios."""

    def test_multiple_sandboxes(self):
        s1 = eryx.Sandbox()
        s2 = eryx.Sandbox()
        r1 = s1.execute('print("one")')
        r2 = s2.execute('print("two")')
        assert r1.stdout == "one"
        assert r2.stdout == "two"

    def test_execute_empty_code(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('')
        assert result.stdout == ""

    def test_execute_only_assignment(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('x = 42')
        assert result.stdout == ""

    def test_large_output(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('print("x" * 10000)')
        assert len(result.stdout) == 10000

    def test_unicode_output(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute('print("Hello 世界")')
        assert "世界" in result.stdout

    def test_multiple_prints(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute("""
print("line 1")
print("line 2")
print("line 3")
""")
        assert result.stdout == "line 1\nline 2\nline 3"

    def test_computation(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute("""
total = sum(range(100))
print(total)
""")
        assert result.stdout == "4950"

    def test_list_comprehension(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute("""
squares = [x**2 for x in range(10)]
print(squares)
""")
        assert "[0, 1, 4, 9, 16, 25, 36, 49, 64, 81]" in result.stdout

    def test_dictionary_operations(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute("""
d = {"a": 1, "b": 2, "c": 3}
print(sorted(d.keys()))
print(sum(d.values()))
""")
        assert "['a', 'b', 'c']" in result.stdout
        assert "6" in result.stdout

    def test_class_definition(self):
        sandbox = eryx.Sandbox()
        result = sandbox.execute("""
class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y
    def __repr__(self):
        return f"Point({self.x}, {self.y})"

p = Point(3, 4)
print(p)
""")
        assert result.stdout == "Point(3, 4)"

    def test_exception_message_content(self):
        sandbox = eryx.Sandbox()
        try:
            sandbox.execute('raise ValueError("custom message 123")')
        except eryx.ExecutionError as e:
            assert "custom message 123" in str(e)
        else:
            pytest.fail("Expected ExecutionError")

    def test_session_complex_workflow(self):
        """Test a realistic multi-step session workflow."""
        session = eryx.Session()

        # Define a data structure
        session.execute("""
data = []
def add_item(name, value):
    data.append({"name": name, "value": value})
""")

        # Add items
        session.execute('add_item("alpha", 10)')
        session.execute('add_item("beta", 20)')
        session.execute('add_item("gamma", 30)')

        # Query
        result = session.execute('print(len(data))')
        assert result.stdout == "3"

        result = session.execute('print(sum(d["value"] for d in data))')
        assert result.stdout == "60"
