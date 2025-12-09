"""
Basic tests for epsilon-python
"""

import pytest


class TestRuntime:
    """Test Runtime class"""

    def test_create_runtime(self):
        """Test creating a runtime"""
        from epsilon import Runtime

        runtime = Runtime()
        assert runtime is not None
        runtime.close()

    def test_runtime_context_manager(self):
        """Test runtime as context manager"""
        from epsilon import Runtime

        with Runtime() as runtime:
            assert runtime is not None

    def test_runtime_with_memory_limit(self):
        """Test creating runtime with memory limit"""
        from epsilon import Runtime

        with Runtime(max_memory_pages=256) as runtime:
            assert runtime is not None


class TestModule:
    """Test Module class"""

    def test_instantiate_module(self, add_wasm):
        """Test instantiating a module"""
        from epsilon import Runtime

        with Runtime() as runtime:
            with runtime.instantiate(add_wasm) as module:
                assert module is not None

    def test_call_function(self, add_wasm):
        """Test calling a function"""
        from epsilon import Runtime

        with Runtime() as runtime:
            with runtime.instantiate(add_wasm) as module:
                result = module.call("add", 5, 37)
                assert result == [42]

    def test_call_function_multiple_times(self, add_wasm):
        """Test calling a function multiple times"""
        from epsilon import Runtime

        with Runtime() as runtime:
            with runtime.instantiate(add_wasm) as module:
                assert module.call("add", 1, 2) == [3]
                assert module.call("add", 100, 200) == [300]
                assert module.call("add", 0, 0) == [0]

    def test_get_export_names(self, add_wasm):
        """Test getting export names"""
        from epsilon import Runtime

        with Runtime() as runtime:
            with runtime.instantiate(add_wasm) as module:
                names = module.get_export_names()
                assert "add" in names

    def test_call_nonexistent_function(self, add_wasm):
        """Test calling a function that doesn't exist"""
        from epsilon import Runtime, EpsilonError

        with Runtime() as runtime:
            with runtime.instantiate(add_wasm) as module:
                with pytest.raises(EpsilonError):
                    module.call("nonexistent")


class TestVersion:
    """Test version functions"""

    def test_version(self):
        """Test getting epsilon version"""
        from epsilon import version

        v = version()
        assert v is not None
        assert isinstance(v, str)

    def test_wrapper_version(self):
        """Test getting wrapper version"""
        from epsilon import wrapper_version

        v = wrapper_version()
        assert v == "0.1.0"
