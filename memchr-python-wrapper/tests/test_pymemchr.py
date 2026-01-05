"""Tests for pymemchr - Python bindings for the memchr Rust library."""

import pytest
import pymemchr


class TestMemchr:
    """Tests for single byte search functions."""

    def test_memchr_found(self):
        """Test memchr finds the first occurrence of a byte."""
        haystack = b"hello world"
        assert pymemchr.memchr(ord('o'), haystack) == 4
        assert pymemchr.memchr(ord('h'), haystack) == 0
        assert pymemchr.memchr(ord('d'), haystack) == 10

    def test_memchr_not_found(self):
        """Test memchr returns None when byte is not found."""
        haystack = b"hello world"
        assert pymemchr.memchr(ord('z'), haystack) is None

    def test_memchr_empty_haystack(self):
        """Test memchr with empty haystack."""
        assert pymemchr.memchr(ord('a'), b"") is None

    def test_memchr2_found(self):
        """Test memchr2 finds first occurrence of either byte."""
        haystack = b"abcdef"
        assert pymemchr.memchr2(ord('c'), ord('e'), haystack) == 2
        assert pymemchr.memchr2(ord('z'), ord('a'), haystack) == 0
        assert pymemchr.memchr2(ord('f'), ord('d'), haystack) == 3

    def test_memchr2_not_found(self):
        """Test memchr2 returns None when neither byte is found."""
        haystack = b"abcdef"
        assert pymemchr.memchr2(ord('x'), ord('y'), haystack) is None

    def test_memchr3_found(self):
        """Test memchr3 finds first occurrence of any of three bytes."""
        haystack = b"abcdef"
        assert pymemchr.memchr3(ord('c'), ord('e'), ord('a'), haystack) == 0
        assert pymemchr.memchr3(ord('x'), ord('y'), ord('f'), haystack) == 5

    def test_memchr3_not_found(self):
        """Test memchr3 returns None when none of the bytes are found."""
        haystack = b"abcdef"
        assert pymemchr.memchr3(ord('x'), ord('y'), ord('z'), haystack) is None


class TestMemrchr:
    """Tests for reverse single byte search functions."""

    def test_memrchr_found(self):
        """Test memrchr finds the last occurrence of a byte."""
        haystack = b"hello world"
        assert pymemchr.memrchr(ord('o'), haystack) == 7
        assert pymemchr.memrchr(ord('l'), haystack) == 9
        assert pymemchr.memrchr(ord('h'), haystack) == 0

    def test_memrchr_not_found(self):
        """Test memrchr returns None when byte is not found."""
        haystack = b"hello world"
        assert pymemchr.memrchr(ord('z'), haystack) is None

    def test_memrchr2_found(self):
        """Test memrchr2 finds last occurrence of either byte."""
        haystack = b"abcabc"
        assert pymemchr.memrchr2(ord('a'), ord('b'), haystack) == 4
        assert pymemchr.memrchr2(ord('c'), ord('x'), haystack) == 5

    def test_memrchr3_found(self):
        """Test memrchr3 finds last occurrence of any of three bytes."""
        haystack = b"abcdef"
        assert pymemchr.memrchr3(ord('a'), ord('b'), ord('c'), haystack) == 2


class TestMemchrIter:
    """Tests for byte search iterator functions."""

    def test_memchr_iter(self):
        """Test memchr_iter finds all occurrences."""
        haystack = b"abracadabra"
        result = pymemchr.memchr_iter(ord('a'), haystack)
        assert result == [0, 3, 5, 7, 10]

    def test_memchr_iter_empty(self):
        """Test memchr_iter with no matches."""
        haystack = b"hello"
        result = pymemchr.memchr_iter(ord('z'), haystack)
        assert result == []

    def test_memchr2_iter(self):
        """Test memchr2_iter finds all occurrences of either byte."""
        haystack = b"aXbXcXabc"
        result = pymemchr.memchr2_iter(ord('a'), ord('b'), haystack)
        assert result == [0, 2, 6, 7]

    def test_memchr3_iter(self):
        """Test memchr3_iter finds all occurrences of any byte."""
        haystack = b"aXbYcZabc"
        result = pymemchr.memchr3_iter(ord('a'), ord('b'), ord('c'), haystack)
        assert result == [0, 2, 4, 6, 7, 8]

    def test_memrchr_iter(self):
        """Test memrchr_iter finds all occurrences in reverse order."""
        haystack = b"abracadabra"
        result = pymemchr.memrchr_iter(ord('a'), haystack)
        assert result == [10, 7, 5, 3, 0]

    def test_memrchr2_iter(self):
        """Test memrchr2_iter finds all occurrences in reverse order."""
        haystack = b"aXbXcXabc"
        result = pymemchr.memrchr2_iter(ord('a'), ord('b'), haystack)
        assert result == [7, 6, 2, 0]

    def test_memrchr3_iter(self):
        """Test memrchr3_iter finds all occurrences in reverse order."""
        haystack = b"aXbYcZabc"
        result = pymemchr.memrchr3_iter(ord('a'), ord('b'), ord('c'), haystack)
        assert result == [8, 7, 6, 4, 2, 0]


class TestMemmem:
    """Tests for substring search functions."""

    def test_memmem_find(self):
        """Test memmem_find finds first substring occurrence."""
        haystack = b"foo bar foo baz foo"
        assert pymemchr.memmem_find(b"foo", haystack) == 0
        assert pymemchr.memmem_find(b"bar", haystack) == 4
        assert pymemchr.memmem_find(b"baz", haystack) == 12

    def test_memmem_find_not_found(self):
        """Test memmem_find returns None when substring is not found."""
        haystack = b"hello world"
        assert pymemchr.memmem_find(b"xyz", haystack) is None

    def test_memmem_find_empty_needle(self):
        """Test memmem_find with empty needle."""
        haystack = b"hello"
        # Empty needle typically matches at position 0
        assert pymemchr.memmem_find(b"", haystack) == 0

    def test_memmem_rfind(self):
        """Test memmem_rfind finds last substring occurrence."""
        haystack = b"foo bar foo baz foo"
        assert pymemchr.memmem_rfind(b"foo", haystack) == 16
        assert pymemchr.memmem_rfind(b"bar", haystack) == 4

    def test_memmem_rfind_not_found(self):
        """Test memmem_rfind returns None when substring is not found."""
        haystack = b"hello world"
        assert pymemchr.memmem_rfind(b"xyz", haystack) is None

    def test_memmem_find_iter(self):
        """Test memmem_find_iter finds all substring occurrences."""
        haystack = b"foo bar foo baz foo"
        result = pymemchr.memmem_find_iter(b"foo", haystack)
        assert result == [0, 8, 16]

    def test_memmem_find_iter_overlapping(self):
        """Test memmem_find_iter with overlapping patterns."""
        haystack = b"aaaa"
        result = pymemchr.memmem_find_iter(b"aa", haystack)
        # memchr doesn't find overlapping - it advances past each match
        assert result == [0, 2]

    def test_memmem_find_iter_empty(self):
        """Test memmem_find_iter with no matches."""
        haystack = b"hello world"
        result = pymemchr.memmem_find_iter(b"xyz", haystack)
        assert result == []


class TestFinder:
    """Tests for the precompiled Finder class."""

    def test_finder_basic(self):
        """Test basic Finder functionality."""
        finder = pymemchr.Finder(b"foo")
        assert finder.find(b"bar foo baz") == 4
        assert finder.find(b"no match here") is None

    def test_finder_multiple_haystacks(self):
        """Test Finder can be reused with multiple haystacks."""
        finder = pymemchr.Finder(b"test")
        assert finder.find(b"this is a test") == 10
        assert finder.find(b"test at start") == 0
        assert finder.find(b"at the end test") == 11
        assert finder.find(b"no match") is None

    def test_finder_find_iter(self):
        """Test Finder.find_iter method."""
        finder = pymemchr.Finder(b"ab")
        result = finder.find_iter(b"ab cd ab ef ab")
        assert result == [0, 6, 12]

    def test_finder_needle(self):
        """Test Finder.needle method returns the needle."""
        finder = pymemchr.Finder(b"hello")
        assert bytes(finder.needle()) == b"hello"


class TestFinderRev:
    """Tests for the precompiled FinderRev class."""

    def test_finder_rev_basic(self):
        """Test basic FinderRev functionality."""
        finder = pymemchr.FinderRev(b"foo")
        assert finder.rfind(b"foo bar foo") == 8

    def test_finder_rev_not_found(self):
        """Test FinderRev returns None when not found."""
        finder = pymemchr.FinderRev(b"xyz")
        assert finder.rfind(b"hello world") is None

    def test_finder_rev_needle(self):
        """Test FinderRev.needle method returns the needle."""
        finder = pymemchr.FinderRev(b"world")
        assert bytes(finder.needle()) == b"world"


class TestLargeInputs:
    """Tests with larger inputs to verify correctness."""

    def test_large_haystack_memchr(self):
        """Test memchr with large haystack."""
        # 1MB of 'a' with a single 'z' in the middle
        size = 1_000_000
        haystack = bytearray(b'a' * size)
        haystack[size // 2] = ord('z')
        haystack = bytes(haystack)

        result = pymemchr.memchr(ord('z'), haystack)
        assert result == size // 2

    def test_large_haystack_memmem(self):
        """Test memmem with large haystack."""
        size = 1_000_000
        haystack = b'x' * (size // 2) + b'needle' + b'x' * (size // 2)
        result = pymemchr.memmem_find(b'needle', haystack)
        assert result == size // 2


class TestBinaryData:
    """Tests with binary data including null bytes."""

    def test_binary_with_nulls(self):
        """Test search works correctly with null bytes."""
        haystack = b'\x00\x01\x02\x00\x03\x04'
        assert pymemchr.memchr(0, haystack) == 0
        assert pymemchr.memrchr(0, haystack) == 3
        assert pymemchr.memchr_iter(0, haystack) == [0, 3]

    def test_memmem_with_nulls(self):
        """Test substring search with null bytes."""
        haystack = b'foo\x00bar\x00foo'
        result = pymemchr.memmem_find_iter(b'foo', haystack)
        assert result == [0, 8]

    def test_binary_needle_with_nulls(self):
        """Test searching for pattern containing null bytes."""
        haystack = b'hello\x00world\x00hello\x00world'
        needle = b'\x00world'
        result = pymemchr.memmem_find_iter(needle, haystack)
        assert result == [5, 17]
