"""Tests for wazero.Runtime"""

import pytest
import wazero


class TestRuntime:
    """Test Runtime creation and management"""

    def test_create_runtime(self):
        """Test creating a runtime"""
        runtime = wazero.Runtime()
        assert runtime is not None
        runtime.close()

    def test_context_manager(self):
        """Test using runtime as context manager"""
        with wazero.Runtime() as runtime:
            assert runtime is not None

    def test_double_close(self):
        """Test that double close doesn't crash"""
        runtime = wazero.Runtime()
        runtime.close()
        runtime.close()  # Should be safe

    def test_instantiate_bytes(self, runtime, add_wasm_bytes):
        """Test instantiating module from bytes"""
        module = runtime.instantiate(add_wasm_bytes)
        assert module is not None
        module.close()

    def test_instantiate_bytearray(self, runtime, add_wasm_bytes):
        """Test instantiating module from bytearray"""
        module = runtime.instantiate(bytearray(add_wasm_bytes))
        assert module is not None
        module.close()

    def test_instantiate_file(self, runtime, add_wasm_file):
        """Test instantiating module from file"""
        module = runtime.instantiate_file(add_wasm_file)
        assert module is not None
        module.close()

    def test_instantiate_invalid_bytes(self, runtime):
        """Test instantiating with invalid WASM data"""
        with pytest.raises(wazero.WazeroError):
            runtime.instantiate(b"not a wasm module")

    def test_instantiate_nonexistent_file(self, runtime):
        """Test instantiating from non-existent file"""
        with pytest.raises(wazero.WazeroError):
            runtime.instantiate_file("/nonexistent/file.wasm")

    def test_instantiate_wrong_type(self, runtime):
        """Test instantiating with wrong type raises TypeError"""
        with pytest.raises(TypeError):
            runtime.instantiate("not bytes")

    def test_instantiate_after_close(self, add_wasm_bytes):
        """Test that using closed runtime raises error"""
        runtime = wazero.Runtime()
        runtime.close()
        with pytest.raises(wazero.WazeroError, match="Runtime is closed"):
            runtime.instantiate(add_wasm_bytes)
