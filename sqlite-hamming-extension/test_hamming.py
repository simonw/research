"""Tests for the hamming_distance SQLite scalar function extension."""

import os
import sqlite3
import struct
import pytest

EXTENSION_PATH = os.path.join(os.path.dirname(__file__), "hamming")


def get_conn():
    """Return an in-memory SQLite connection with the extension loaded."""
    conn = sqlite3.connect(":memory:")
    conn.enable_load_extension(True)
    conn.load_extension(EXTENSION_PATH)
    return conn


# ---------------------------------------------------------------------------
# Basic correctness tests
# ---------------------------------------------------------------------------

class TestHammingDistanceBasic:

    def test_identical_vectors(self):
        conn = get_conn()
        blob = b"\xff" * 128
        result = conn.execute(
            "SELECT hamming_distance(?, ?)", (blob, blob)
        ).fetchone()[0]
        assert result == 0

    def test_completely_different_bytes(self):
        """0x00 vs 0xFF for every byte -> every bit differs."""
        conn = get_conn()
        a = b"\x00" * 128
        b_vec = b"\xff" * 128
        result = conn.execute(
            "SELECT hamming_distance(?, ?)", (a, b_vec)
        ).fetchone()[0]
        assert result == 1024  # 128 bytes * 8 bits

    def test_single_byte_known_distance(self):
        conn = get_conn()
        # 0b10110110  vs  0b10001110
        # XOR = 0b00111000 -> popcount = 3
        a = bytes([0b10110110])
        b_vec = bytes([0b10001110])
        result = conn.execute(
            "SELECT hamming_distance(?, ?)", (a, b_vec)
        ).fetchone()[0]
        assert result == 3

    def test_article_example(self):
        """The 8-bit example from the article: distance = 3."""
        conn = get_conn()
        # Vector A: 1 0 1 1 0 1 1 0 = 0xB6
        # Vector B: 1 0 0 1 1 0 1 0 = 0x9A
        a = bytes([0b10110110])
        b_vec = bytes([0b10011010])
        result = conn.execute(
            "SELECT hamming_distance(?, ?)", (a, b_vec)
        ).fetchone()[0]
        assert result == 3

    def test_multi_byte_vector(self):
        conn = get_conn()
        # 16-byte vectors: first 8 bytes identical, last 8 all-different
        a = b"\xAA" * 8 + b"\x00" * 8
        b_vec = b"\xAA" * 8 + b"\xFF" * 8
        result = conn.execute(
            "SELECT hamming_distance(?, ?)", (a, b_vec)
        ).fetchone()[0]
        assert result == 64  # 8 bytes * 8 bits differ

    def test_non_8_aligned_length(self):
        """Vectors whose length is not a multiple of 8."""
        conn = get_conn()
        a = b"\x00" * 13
        b_vec = b"\xFF" * 13
        result = conn.execute(
            "SELECT hamming_distance(?, ?)", (a, b_vec)
        ).fetchone()[0]
        assert result == 13 * 8


# ---------------------------------------------------------------------------
# Error handling tests
# ---------------------------------------------------------------------------

class TestHammingDistanceErrors:

    def test_different_lengths(self):
        conn = get_conn()
        with pytest.raises(sqlite3.OperationalError, match="same length"):
            conn.execute(
                "SELECT hamming_distance(?, ?)",
                (b"\x00" * 10, b"\x00" * 12),
            ).fetchone()

    def test_null_input_returns_null(self):
        conn = get_conn()
        result = conn.execute(
            "SELECT hamming_distance(NULL, ?)", (b"\x00" * 8,)
        ).fetchone()[0]
        assert result is None

    def test_empty_blobs(self):
        conn = get_conn()
        result = conn.execute(
            "SELECT hamming_distance(?, ?)", (b"", b"")
        ).fetchone()[0]
        assert result == 0


# ---------------------------------------------------------------------------
# Symmetry and triangle inequality
# ---------------------------------------------------------------------------

class TestHammingDistanceProperties:

    def test_symmetry(self):
        conn = get_conn()
        import os as _os
        a = _os.urandom(128)
        b_vec = _os.urandom(128)
        d1 = conn.execute(
            "SELECT hamming_distance(?, ?)", (a, b_vec)
        ).fetchone()[0]
        d2 = conn.execute(
            "SELECT hamming_distance(?, ?)", (b_vec, a)
        ).fetchone()[0]
        assert d1 == d2

    def test_triangle_inequality(self):
        conn = get_conn()
        import os as _os
        a = _os.urandom(128)
        b_vec = _os.urandom(128)
        c = _os.urandom(128)
        d_ab = conn.execute(
            "SELECT hamming_distance(?, ?)", (a, b_vec)
        ).fetchone()[0]
        d_bc = conn.execute(
            "SELECT hamming_distance(?, ?)", (b_vec, c)
        ).fetchone()[0]
        d_ac = conn.execute(
            "SELECT hamming_distance(?, ?)", (a, c)
        ).fetchone()[0]
        assert d_ac <= d_ab + d_bc
