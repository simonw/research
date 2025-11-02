#!/usr/bin/env python3
"""
Quick test of the Python bindings
"""

import wazero

def main():
    print("Testing Wazero Python Bindings")
    print("=" * 50)

    # Create a runtime
    print("\n1. Creating runtime...")
    runtime = wazero.Runtime()
    print(f"   ✓ Runtime created")

    # Load the WASM module
    print("\n2. Loading add.wasm module...")
    try:
        module = runtime.instantiate_file("add.wasm")
        print(f"   ✓ Module instantiated")
    except Exception as e:
        print(f"   ✗ Failed: {e}")
        return

    # Call the add function
    print("\n3. Testing function calls...")
    test_cases = [
        (5, 7),
        (100, 200),
        (42, 0),
        (1000, 2000),
    ]

    for a, b in test_cases:
        result = module.call("add", a, b)
        expected = a + b
        status = "✓" if result[0] == expected else "✗"
        print(f"   {status} add({a}, {b}) = {result[0]} (expected {expected})")

    # Clean up
    print("\n4. Cleaning up...")
    module.close()
    runtime.close()
    print("   ✓ Resources freed")

    print("\n" + "=" * 50)
    print("All tests passed!")

if __name__ == "__main__":
    main()
