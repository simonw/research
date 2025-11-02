"""Tests for wazero.Module"""

import pytest
import wazero


class TestModule:
    """Test Module instantiation and function calls"""

    def test_call_function(self, runtime, add_wasm_bytes):
        """Test calling an exported function"""
        module = runtime.instantiate(add_wasm_bytes)
        result = module.call("add", 5, 7)
        assert result == [12]
        module.close()

    def test_call_multiple_times(self, runtime, add_wasm_bytes):
        """Test calling function multiple times"""
        module = runtime.instantiate(add_wasm_bytes)

        assert module.call("add", 1, 2) == [3]
        assert module.call("add", 10, 20) == [30]
        assert module.call("add", 100, 200) == [300]

        module.close()

    def test_call_with_zero(self, runtime, add_wasm_bytes):
        """Test calling function with zero arguments"""
        module = runtime.instantiate(add_wasm_bytes)
        assert module.call("add", 42, 0) == [42]
        assert module.call("add", 0, 42) == [42]
        module.close()

    def test_call_large_numbers(self, runtime, add_wasm_bytes):
        """Test calling function with large numbers"""
        module = runtime.instantiate(add_wasm_bytes)
        result = module.call("add", 1000000, 2000000)
        assert result == [3000000]
        module.close()

    def test_call_nonexistent_function(self, runtime, add_wasm_bytes):
        """Test calling a non-existent function"""
        module = runtime.instantiate(add_wasm_bytes)
        with pytest.raises(wazero.WazeroError, match="not found"):
            module.call("nonexistent", 1, 2)
        module.close()

    def test_call_after_close(self, runtime, add_wasm_bytes):
        """Test calling function after module is closed"""
        module = runtime.instantiate(add_wasm_bytes)
        module.close()
        with pytest.raises(wazero.WazeroError, match="Module is closed"):
            module.call("add", 1, 2)

    def test_context_manager(self, runtime, add_wasm_bytes):
        """Test using module as context manager"""
        with runtime.instantiate(add_wasm_bytes) as module:
            result = module.call("add", 10, 20)
            assert result == [30]

    def test_multiple_modules(self, runtime, add_wasm_bytes):
        """Test creating multiple modules from same runtime"""
        module1 = runtime.instantiate(add_wasm_bytes)
        module2 = runtime.instantiate(add_wasm_bytes)

        result1 = module1.call("add", 1, 2)
        result2 = module2.call("add", 3, 4)

        assert result1 == [3]
        assert result2 == [7]

        module1.close()
        module2.close()

    def test_double_close(self, runtime, add_wasm_bytes):
        """Test that double close doesn't crash"""
        module = runtime.instantiate(add_wasm_bytes)
        module.close()
        module.close()  # Should be safe


class TestModuleIntegration:
    """Integration tests for complex scenarios"""

    def test_sequential_operations(self, runtime, add_wasm_bytes):
        """Test a sequence of operations"""
        with runtime.instantiate(add_wasm_bytes) as module:
            # Simulate accumulator
            total = 0
            for i in range(10):
                result = module.call("add", total, i)
                total = result[0]

            assert total == sum(range(10))  # 0+1+2+...+9 = 45

    def test_performance(self, runtime, add_wasm_bytes):
        """Test that we can make many calls efficiently"""
        with runtime.instantiate(add_wasm_bytes) as module:
            # Make 1000 calls
            for i in range(1000):
                result = module.call("add", i, i)
                assert result[0] == i * 2
