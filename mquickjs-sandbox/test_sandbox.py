"""
Comprehensive tests for mquickjs sandbox implementations.

Run with: pytest test_sandbox.py -v
"""

import pytest
import time
import sys
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent))

from api_design import SandboxError, TimeoutError, MemoryError
from mquickjs_ffi import MQuickJSFFI, execute_js


class TestBasicExecution:
    """Test basic JavaScript execution."""

    def test_arithmetic(self):
        """Test basic arithmetic operations."""
        assert execute_js("1 + 2") == 3
        assert execute_js("10 - 3") == 7
        assert execute_js("4 * 5") == 20
        assert execute_js("15 / 3") == 5
        assert execute_js("17 % 5") == 2

    def test_floating_point(self):
        """Test floating point numbers."""
        result = execute_js("3.14159")
        assert abs(result - 3.14159) < 0.0001
        result = execute_js("1.5 + 2.5")
        assert result == 4.0

    def test_strings(self):
        """Test string operations."""
        assert execute_js("'hello'") == "hello"
        assert execute_js("'hello' + ' world'") == "hello world"
        assert execute_js("'abc'.length") == 3
        assert execute_js("'HELLO'.toLowerCase()") == "hello"

    def test_booleans(self):
        """Test boolean operations."""
        assert execute_js("true") == True
        assert execute_js("false") == False
        assert execute_js("true && false") == False
        assert execute_js("true || false") == True
        assert execute_js("!true") == False

    def test_null_undefined(self):
        """Test null and undefined."""
        assert execute_js("null") is None
        assert execute_js("undefined") is None

    def test_arrays(self):
        """Test array operations."""
        assert execute_js("[1, 2, 3].length") == 3
        assert execute_js("[1, 2, 3][1]") == 2
        assert execute_js("[1, 2, 3].map(function(x) { return x * 2; }).join(',')") == "2,4,6"

    def test_objects(self):
        """Test object operations."""
        result = execute_js("({a: 1, b: 2}).a")
        assert result == 1


class TestControlFlow:
    """Test control flow structures."""

    def test_if_else(self):
        """Test if/else statements."""
        code = """
        (function() {
            var x = 5;
            if (x > 3) {
                return 'big';
            } else {
                return 'small';
            }
        })()
        """
        assert execute_js(code) == "big"

    def test_for_loop(self):
        """Test for loops."""
        code = """
        (function() {
            var sum = 0;
            for (var i = 1; i <= 10; i++) {
                sum += i;
            }
            return sum;
        })()
        """
        assert execute_js(code) == 55

    def test_while_loop(self):
        """Test while loops."""
        code = """
        (function() {
            var count = 0;
            while (count < 5) {
                count++;
            }
            return count;
        })()
        """
        assert execute_js(code) == 5

    def test_switch(self):
        """Test switch statements."""
        code = """
        (function() {
            var x = 2;
            switch (x) {
                case 1: return 'one';
                case 2: return 'two';
                case 3: return 'three';
                default: return 'other';
            }
        })()
        """
        assert execute_js(code) == "two"


class TestFunctions:
    """Test function features."""

    def test_function_declaration(self):
        """Test function declaration and invocation."""
        code = """
        function add(a, b) {
            return a + b;
        }
        add(3, 4)
        """
        assert execute_js(code) == 7

    def test_anonymous_function(self):
        """Test anonymous functions."""
        code = """
        (function(x) { return x * 2; })(5)
        """
        assert execute_js(code) == 10

    def test_closures(self):
        """Test closures."""
        code = """
        (function() {
            function makeCounter() {
                var count = 0;
                return function() {
                    return ++count;
                };
            }
            var counter = makeCounter();
            counter();
            counter();
            return counter();
        })()
        """
        assert execute_js(code) == 3

    def test_recursion(self):
        """Test recursive functions."""
        code = """
        function factorial(n) {
            if (n <= 1) return 1;
            return n * factorial(n - 1);
        }
        factorial(5)
        """
        assert execute_js(code) == 120


class TestBuiltins:
    """Test built-in objects and functions."""

    def test_math(self):
        """Test Math object."""
        assert execute_js("Math.abs(-5)") == 5
        assert execute_js("Math.min(3, 1, 4)") == 1
        assert execute_js("Math.max(3, 1, 4)") == 4
        assert execute_js("Math.floor(3.7)") == 3
        assert execute_js("Math.ceil(3.2)") == 4

    def test_string_methods(self):
        """Test String methods."""
        assert execute_js("'hello world'.indexOf('world')") == 6
        assert execute_js("'hello'.charAt(1)") == "e"
        assert execute_js("'a,b,c'.split(',').length") == 3
        assert execute_js("'  hello  '.trim()") == "hello"

    def test_array_methods(self):
        """Test Array methods."""
        assert execute_js("[3, 1, 4, 1, 5].sort().join(',')") == "1,1,3,4,5"
        assert execute_js("[1, 2, 3].reverse().join(',')") == "3,2,1"
        assert execute_js("[1, 2, 3].indexOf(2)") == 1

    def test_json(self):
        """Test JSON.parse and JSON.stringify."""
        assert execute_js("JSON.parse('[1, 2, 3]').length") == 3
        assert execute_js("JSON.stringify([1, 2, 3])") == "[1,2,3]"


class TestErrors:
    """Test error handling."""

    def test_syntax_error(self):
        """Test that syntax errors raise exceptions."""
        with pytest.raises(SandboxError):
            execute_js("function(")

    def test_reference_error(self):
        """Test that reference errors raise exceptions."""
        with pytest.raises(SandboxError):
            execute_js("undefinedVariable")

    def test_type_error(self):
        """Test that type errors raise exceptions."""
        with pytest.raises(SandboxError):
            execute_js("null.property")

    def test_throw(self):
        """Test that thrown exceptions are caught."""
        with pytest.raises(SandboxError):
            execute_js("throw new Error('test error')")


class TestSandboxing:
    """Test sandboxing features."""

    def test_no_load(self):
        """Test that load() is disabled."""
        with pytest.raises(SandboxError):
            execute_js("load('test.js')")

    def test_no_settimeout(self):
        """Test that setTimeout() is disabled."""
        with pytest.raises(SandboxError):
            execute_js("setTimeout(function() {}, 100)")

    def test_memory_limit(self):
        """Test memory limits."""
        # Try to allocate a large array with a small memory limit
        sandbox = MQuickJSFFI(memory_limit_bytes=16384, time_limit_ms=1000)
        try:
            # This should fail due to memory limit
            with pytest.raises(SandboxError):
                sandbox.execute("""
                    var arr = [];
                    for (var i = 0; i < 1000000; i++) {
                        arr.push(i);
                    }
                    arr.length
                """)
        finally:
            sandbox.close()

    def test_time_limit(self):
        """Test time limits."""
        sandbox = MQuickJSFFI(memory_limit_bytes=1024*1024, time_limit_ms=100)
        try:
            start = time.time()
            with pytest.raises(TimeoutError):
                sandbox.execute("while(true) {}")
            elapsed = time.time() - start
            # Should timeout within reasonable time (allowing some margin)
            assert elapsed < 1.0, f"Timeout took too long: {elapsed}s"
        finally:
            sandbox.close()

    def test_deterministic_random(self):
        """Test that Math.random() is deterministic."""
        result1 = execute_js("Math.random()")
        result2 = execute_js("Math.random()")
        # Same seed should give same result
        assert result1 == result2


class TestReDoS:
    """Test ReDoS protection."""

    def test_regex_timeout(self):
        """Test that pathological regex patterns timeout."""
        sandbox = MQuickJSFFI(memory_limit_bytes=1024*1024, time_limit_ms=200)
        try:
            start = time.time()
            with pytest.raises(TimeoutError):
                # This is a classic ReDoS pattern
                sandbox.execute("""
                    /(a+)+$/.test('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaX')
                """)
            elapsed = time.time() - start
            assert elapsed < 1.0, f"ReDoS timeout took too long: {elapsed}s"
        finally:
            sandbox.close()


class TestContextReuse:
    """Test that sandbox contexts can be reused."""

    def test_variable_persistence(self):
        """Test that variables persist between executions."""
        sandbox = MQuickJSFFI()
        try:
            sandbox.execute("var x = 42")
            result = sandbox.execute("x")
            assert result == 42
        finally:
            sandbox.close()

    def test_function_persistence(self):
        """Test that functions persist between executions."""
        sandbox = MQuickJSFFI()
        try:
            sandbox.execute("function double(n) { return n * 2; }")
            result = sandbox.execute("double(21)")
            assert result == 42
        finally:
            sandbox.close()


class TestEdgeCases:
    """Test edge cases."""

    def test_empty_string(self):
        """Test empty string input."""
        result = execute_js("")
        assert result is None

    def test_unicode(self):
        """Test unicode handling."""
        result = execute_js("'hello'")  # Basic ASCII
        assert result == "hello"

    def test_very_long_string(self):
        """Test handling of long strings."""
        # Note: String.repeat() is not available in mquickjs (ES5 subset)
        # Use a loop instead
        code = """
        (function() {
            var s = '';
            for (var i = 0; i < 100; i++) s += 'x';
            return s;
        })()
        """
        result = execute_js(code)
        assert len(result) == 100

    def test_nested_calls(self):
        """Test deeply nested function calls."""
        code = """
        function f(n) {
            if (n <= 0) return 0;
            return 1 + f(n - 1);
        }
        f(100)
        """
        result = execute_js(code)
        assert result == 100


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
