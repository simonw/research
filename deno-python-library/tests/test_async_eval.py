"""Tests for asynchronous JavaScript evaluation."""

import pytest
from denobox import AsyncDenoBox


class TestAsyncDenoBox:
    """Test asynchronous DenoBox functionality."""

    async def test_simple_eval(self):
        """Test evaluating a simple expression."""
        async with AsyncDenoBox() as box:
            result = await box.eval("1 + 1")
            assert result == 2

    async def test_string_result(self):
        """Test evaluating code that returns a string."""
        async with AsyncDenoBox() as box:
            result = await box.eval("'hello' + ' ' + 'world'")
            assert result == "hello world"

    async def test_array_result(self):
        """Test evaluating code that returns an array."""
        async with AsyncDenoBox() as box:
            result = await box.eval("[1, 2, 3].map(x => x * 2)")
            assert result == [2, 4, 6]

    async def test_object_result(self):
        """Test evaluating code that returns an object."""
        async with AsyncDenoBox() as box:
            result = await box.eval("({name: 'test', value: 42})")
            assert result == {"name": "test", "value": 42}

    async def test_null_result(self):
        """Test evaluating code that returns null."""
        async with AsyncDenoBox() as box:
            result = await box.eval("null")
            assert result is None

    async def test_undefined_result(self):
        """Test evaluating code that returns undefined."""
        async with AsyncDenoBox() as box:
            result = await box.eval("undefined")
            assert result is None

    async def test_boolean_result(self):
        """Test evaluating code that returns boolean."""
        async with AsyncDenoBox() as box:
            assert await box.eval("true") is True
            assert await box.eval("false") is False

    async def test_multiple_evals(self):
        """Test multiple evaluations in same session."""
        async with AsyncDenoBox() as box:
            await box.eval("var x = 10")
            await box.eval("var y = 20")
            result = await box.eval("x + y")
            assert result == 30

    async def test_error_handling(self):
        """Test that JavaScript errors are properly raised."""
        async with AsyncDenoBox() as box:
            with pytest.raises(Exception) as exc_info:
                await box.eval("throw new Error('test error')")
            assert "test error" in str(exc_info.value)

    async def test_syntax_error(self):
        """Test that syntax errors are properly raised."""
        async with AsyncDenoBox() as box:
            with pytest.raises(Exception):
                await box.eval("this is not valid javascript {{{")

    async def test_context_manager_cleanup(self):
        """Test that context manager properly cleans up."""
        box = AsyncDenoBox()
        await box.start()
        assert await box.eval("1 + 1") == 2
        await box.stop()
        # Should not be able to eval after stop
        with pytest.raises(Exception):
            await box.eval("1 + 1")

    async def test_promise_resolution(self):
        """Test that promises are automatically resolved."""
        async with AsyncDenoBox() as box:
            result = await box.eval("Promise.resolve(42)")
            assert result == 42

    async def test_async_function_result(self):
        """Test evaluating async functions."""
        async with AsyncDenoBox() as box:
            result = await box.eval("(async () => { return 'async result'; })()")
            assert result == "async result"

    async def test_concurrent_evals(self):
        """Test concurrent evaluations using asyncio.gather."""
        import asyncio

        async with AsyncDenoBox() as box:
            # Run multiple evals concurrently
            results = await asyncio.gather(
                box.eval("1 + 1"),
                box.eval("2 + 2"),
                box.eval("3 + 3"),
            )
            assert results == [2, 4, 6]
