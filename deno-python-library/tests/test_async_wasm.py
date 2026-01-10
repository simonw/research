"""Tests for asynchronous WebAssembly loading and execution."""

import pytest
import asyncio
from pathlib import Path
from denobox import AsyncDenoBox


FIXTURES_DIR = Path(__file__).parent / "fixtures"
MATH_WASM = FIXTURES_DIR / "math.wasm"


class TestAsyncDenoBoxWasm:
    """Test asynchronous WASM functionality."""

    async def test_load_wasm_module(self):
        """Test loading a WASM module."""
        async with AsyncDenoBox() as box:
            module = await box.load_wasm(str(MATH_WASM))
            assert module is not None
            assert "add" in module.exports
            assert "multiply" in module.exports

    async def test_call_wasm_add(self):
        """Test calling WASM add function."""
        async with AsyncDenoBox() as box:
            module = await box.load_wasm(str(MATH_WASM))
            result = await module.call("add", 3, 4)
            assert result == 7

    async def test_call_wasm_multiply(self):
        """Test calling WASM multiply function."""
        async with AsyncDenoBox() as box:
            module = await box.load_wasm(str(MATH_WASM))
            result = await module.call("multiply", 5, 6)
            assert result == 30

    async def test_multiple_calls(self):
        """Test multiple WASM calls."""
        async with AsyncDenoBox() as box:
            module = await box.load_wasm(str(MATH_WASM))
            assert await module.call("add", 1, 2) == 3
            assert await module.call("add", 10, 20) == 30
            assert await module.call("multiply", 3, 4) == 12

    async def test_load_multiple_modules(self):
        """Test loading the same module multiple times."""
        async with AsyncDenoBox() as box:
            module1 = await box.load_wasm(str(MATH_WASM))
            module2 = await box.load_wasm(str(MATH_WASM))
            assert module1.module_id != module2.module_id
            assert await module1.call("add", 1, 1) == 2
            assert await module2.call("add", 2, 2) == 4

    async def test_nonexistent_function(self):
        """Test calling a non-existent WASM function."""
        async with AsyncDenoBox() as box:
            module = await box.load_wasm(str(MATH_WASM))
            with pytest.raises(Exception) as exc_info:
                await module.call("nonexistent", 1, 2)
            assert "nonexistent" in str(exc_info.value).lower()

    async def test_nonexistent_module(self):
        """Test loading a non-existent WASM file."""
        async with AsyncDenoBox() as box:
            with pytest.raises(Exception):
                await box.load_wasm("/path/to/nonexistent.wasm")

    async def test_unload_module(self):
        """Test unloading a WASM module."""
        async with AsyncDenoBox() as box:
            module = await box.load_wasm(str(MATH_WASM))
            assert await module.call("add", 1, 1) == 2
            await module.unload()
            # Should fail after unload
            with pytest.raises(Exception):
                await module.call("add", 1, 1)

    async def test_concurrent_calls(self):
        """Test concurrent WASM calls."""
        async with AsyncDenoBox() as box:
            module = await box.load_wasm(str(MATH_WASM))
            results = await asyncio.gather(
                module.call("add", 1, 2),
                module.call("add", 3, 4),
                module.call("multiply", 5, 6),
            )
            assert results == [3, 7, 30]
