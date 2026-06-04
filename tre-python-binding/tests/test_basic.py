"""Basic tests for the TRE Python binding.

Red/green TDD: these are written before the implementation. They define the
API surface for the minimal subset we need.
"""
import pytest

import tre_py


def test_compile_returns_pattern():
    pat = tre_py.compile(r"hello")
    assert pat is not None


def test_search_finds_match():
    pat = tre_py.compile(r"hello")
    m = pat.search("say hello world")
    assert m is not None
    assert m.span() == (4, 9)
    assert m.group(0) == "hello"


def test_search_returns_none_when_no_match():
    pat = tre_py.compile(r"hello")
    assert pat.search("goodbye") is None


def test_search_anchored():
    pat = tre_py.compile(r"^hello$")
    assert pat.search("hello") is not None
    assert pat.search("say hello") is None


def test_groups():
    # POSIX extended: parentheses for grouping/capture.
    pat = tre_py.compile(r"(a+)(b+)")
    m = pat.search("xxxaaabbbxxx")
    assert m is not None
    assert m.group(0) == "aaabbb"
    assert m.group(1) == "aaa"
    assert m.group(2) == "bbb"
    assert m.span(1) == (3, 6)
    assert m.span(2) == (6, 9)


def test_alternation():
    pat = tre_py.compile(r"cat|dog")
    assert pat.search("I have a cat").group(0) == "cat"
    assert pat.search("I have a dog").group(0) == "dog"
    assert pat.search("I have a fish") is None


def test_invalid_pattern_raises():
    with pytest.raises(tre_py.TreError):
        tre_py.compile(r"(unclosed")


def test_unicode_pattern():
    # TRE was built with wide-char support; should handle non-ASCII.
    pat = tre_py.compile(r"café")
    m = pat.search("the café is open")
    assert m is not None
    assert m.group(0) == "café"


def test_quantifiers():
    pat = tre_py.compile(r"a{3,5}")
    assert pat.search("aa") is None
    assert pat.search("aaa").group(0) == "aaa"
    assert pat.search("aaaaaaaa").group(0) == "aaaaa"


def test_case_insensitive():
    pat = tre_py.compile(r"hello", flags=tre_py.IGNORECASE)
    assert pat.search("HELLO WORLD").group(0) == "HELLO"
