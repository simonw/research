#!/usr/bin/env python3
"""
Test program for uniprof: Fibonacci calculation with different methods.
This tests CPU-intensive recursive and iterative operations.
"""

import time
import sys

def fibonacci_recursive(n):
    """Recursive fibonacci - intentionally inefficient for profiling."""
    if n <= 1:
        return n
    return fibonacci_recursive(n - 1) + fibonacci_recursive(n - 2)

def fibonacci_iterative(n):
    """Iterative fibonacci - more efficient."""
    if n <= 1:
        return n
    a, b = 0, 1
    for _ in range(2, n + 1):
        a, b = b, a + b
    return b

def fibonacci_memoized(n, memo=None):
    """Memoized fibonacci - efficient recursion."""
    if memo is None:
        memo = {}
    if n in memo:
        return memo[n]
    if n <= 1:
        return n
    memo[n] = fibonacci_memoized(n - 1, memo) + fibonacci_memoized(n - 2, memo)
    return memo[n]

def do_some_work():
    """Simulate various CPU-intensive operations."""
    # Recursive fibonacci (slow)
    print("Computing fibonacci_recursive(30)...")
    start = time.time()
    result = fibonacci_recursive(30)
    elapsed = time.time() - start
    print(f"Result: {result}, Time: {elapsed:.3f}s")

    # Iterative fibonacci (fast)
    print("\nComputing fibonacci_iterative(10000)...")
    start = time.time()
    result = fibonacci_iterative(10000)
    elapsed = time.time() - start
    print(f"Result length: {len(str(result))} digits, Time: {elapsed:.3f}s")

    # Memoized fibonacci (efficient recursion)
    print("\nComputing fibonacci_memoized(500)...")
    start = time.time()
    result = fibonacci_memoized(500)
    elapsed = time.time() - start
    print(f"Result length: {len(str(result))} digits, Time: {elapsed:.3f}s")

    # Some list operations
    print("\nPerforming list operations...")
    start = time.time()
    data = [i ** 2 for i in range(100000)]
    filtered = [x for x in data if x % 2 == 0]
    summed = sum(filtered)
    elapsed = time.time() - start
    print(f"Sum: {summed}, Time: {elapsed:.3f}s")

if __name__ == "__main__":
    print("Starting CPU-intensive operations for profiling...")
    do_some_work()
    print("\nDone!")
