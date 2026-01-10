#!/usr/bin/env python3
"""Test basic Denobox functionality."""

from denobox import Denobox, DenoboxError

print("=== Testing Basic Denobox JavaScript Eval ===\n")

with Denobox() as box:
    # Test 1: Simple arithmetic
    print("Test 1: Simple arithmetic (1 + 1)")
    result = box.eval("1 + 1")
    print(f"  Result: {result}")
    print(f"  Type: {type(result)}")
    print()

    # Test 2: String manipulation
    print("Test 2: String manipulation")
    result = box.eval("'Hello, ' + 'World!'")
    print(f"  Result: {result}")
    print()

    # Test 3: Array operations
    print("Test 3: Array operations")
    result = box.eval("[1, 2, 3, 4, 5].map(x => x * 2)")
    print(f"  Result: {result}")
    print()

    # Test 4: Object creation
    print("Test 4: Object creation")
    result = box.eval("({name: 'Alice', age: 30})")
    print(f"  Result: {result}")
    print()

    # Test 5: Function definition and call
    print("Test 5: Function definition and call")
    result = box.eval("""
        function factorial(n) {
            if (n <= 1) return 1;
            return n * factorial(n - 1);
        }
        factorial(5)
    """)
    print(f"  Result: {result}")
    print()

    # Test 6: JSON parsing
    print("Test 6: JSON operations")
    result = box.eval('JSON.stringify({a: 1, b: [2, 3, 4]})')
    print(f"  Result: {result}")
    print()

    # Test 7: Error handling
    print("Test 7: Error handling")
    try:
        box.eval("throw new Error('This is an error')")
    except DenoboxError as e:
        print(f"  Caught DenoboxError: {e}")
    print()

print("=== All basic tests completed ===")
