"""Pytest configuration and fixtures"""

import pytest
from pathlib import Path


@pytest.fixture
def add_wasm_bytes():
    """Return the bytes for a simple add(i32, i32) -> i32 WASM module"""
    return bytes([
        0x00, 0x61, 0x73, 0x6d,  # WASM magic
        0x01, 0x00, 0x00, 0x00,  # WASM version 1
        # Type section
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
        # Function section
        0x03, 0x02, 0x01, 0x00,
        # Export section
        0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
        # Code section
        0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
    ])


@pytest.fixture
def add_wasm_file(tmp_path, add_wasm_bytes):
    """Create a temporary WASM file"""
    wasm_file = tmp_path / "add.wasm"
    wasm_file.write_bytes(add_wasm_bytes)
    return wasm_file


@pytest.fixture
def runtime():
    """Create a wazero runtime"""
    import wazero
    rt = wazero.Runtime()
    yield rt
    rt.close()
