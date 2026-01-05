"""
pymemchr_c - Python bindings for optimized byte and substring search functions in C

This module provides high-performance byte and substring search functions
with SIMD optimizations on supported platforms (x86_64, aarch64).

Functions:
    memchr(needle, haystack) - Find first byte occurrence
    memchr2(n1, n2, haystack) - Find first of two bytes
    memchr3(n1, n2, n3, haystack) - Find first of three bytes
    memrchr(needle, haystack) - Find last byte occurrence
    memrchr2(n1, n2, haystack) - Find last of two bytes
    memrchr3(n1, n2, n3, haystack) - Find last of three bytes
    memchr_iter(needle, haystack) - Find all byte occurrences
    memmem_find(needle, haystack) - Find first substring
    memmem_rfind(needle, haystack) - Find last substring
    memmem_find_iter(needle, haystack) - Find all substrings

Classes:
    Finder - Precompiled substring finder for repeated searches
    FinderRev - Precompiled reverse substring finder
"""

from pymemchr_c._pymemchr_c import (
    # Single byte search
    memchr,
    memchr2,
    memchr3,
    # Reverse single byte search
    memrchr,
    memrchr2,
    memrchr3,
    # Iterator functions
    memchr_iter,
    memchr2_iter,
    memchr3_iter,
    memrchr_iter,
    memrchr2_iter,
    memrchr3_iter,
    # Substring search
    memmem_find,
    memmem_rfind,
    memmem_find_iter,
    # Classes
    Finder,
    FinderRev,
)

__version__ = "0.1.0"
__all__ = [
    # Single byte search
    "memchr",
    "memchr2",
    "memchr3",
    # Reverse single byte search
    "memrchr",
    "memrchr2",
    "memrchr3",
    # Iterator functions
    "memchr_iter",
    "memchr2_iter",
    "memchr3_iter",
    "memrchr_iter",
    "memrchr2_iter",
    "memrchr3_iter",
    # Substring search
    "memmem_find",
    "memmem_rfind",
    "memmem_find_iter",
    # Classes
    "Finder",
    "FinderRev",
]
