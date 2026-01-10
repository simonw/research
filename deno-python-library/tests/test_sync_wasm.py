"""Tests for synchronous WebAssembly loading and execution."""

import pytest
from pathlib import Path
from denobox import DenoBox


FIXTURES_DIR = Path(__file__).parent / "fixtures"
MATH_WASM = FIXTURES_DIR / "math.wasm"


class TestDenoBoxWasmSync:
    """Test synchronous WASM functionality."""

    def test_load_wasm_module(self):
        """Test loading a WASM module."""
        with DenoBox() as box:
            module = box.load_wasm(str(MATH_WASM))
            assert module is not None
            assert "add" in module.exports
            assert "multiply" in module.exports

    def test_call_wasm_add(self):
        """Test calling WASM add function."""
        with DenoBox() as box:
            module = box.load_wasm(str(MATH_WASM))
            result = module.call("add", 3, 4)
            assert result == 7

    def test_call_wasm_multiply(self):
        """Test calling WASM multiply function."""
        with DenoBox() as box:
            module = box.load_wasm(str(MATH_WASM))
            result = module.call("multiply", 5, 6)
            assert result == 30

    def test_multiple_calls(self):
        """Test multiple WASM calls."""
        with DenoBox() as box:
            module = box.load_wasm(str(MATH_WASM))
            assert module.call("add", 1, 2) == 3
            assert module.call("add", 10, 20) == 30
            assert module.call("multiply", 3, 4) == 12

    def test_load_multiple_modules(self):
        """Test loading the same module multiple times."""
        with DenoBox() as box:
            module1 = box.load_wasm(str(MATH_WASM))
            module2 = box.load_wasm(str(MATH_WASM))
            assert module1.module_id != module2.module_id
            assert module1.call("add", 1, 1) == 2
            assert module2.call("add", 2, 2) == 4

    def test_nonexistent_function(self):
        """Test calling a non-existent WASM function."""
        with DenoBox() as box:
            module = box.load_wasm(str(MATH_WASM))
            with pytest.raises(Exception) as exc_info:
                module.call("nonexistent", 1, 2)
            assert "nonexistent" in str(exc_info.value).lower()

    def test_nonexistent_module(self):
        """Test loading a non-existent WASM file."""
        with DenoBox() as box:
            with pytest.raises(Exception):
                box.load_wasm("/path/to/nonexistent.wasm")

    def test_unload_module(self):
        """Test unloading a WASM module."""
        with DenoBox() as box:
            module = box.load_wasm(str(MATH_WASM))
            assert module.call("add", 1, 1) == 2
            module.unload()
            # Should fail after unload
            with pytest.raises(Exception):
                module.call("add", 1, 1)
