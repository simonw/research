"""
Pytest configuration and fixtures for epsilon-python tests
"""

import pytest
from pathlib import Path


# Simple add.wasm module in WAT format compiled to bytes
# (module
#   (func (export "add") (param i32 i32) (result i32)
#     local.get 0
#     local.get 1
#     i32.add))
ADD_WASM = bytes([
    0x00, 0x61, 0x73, 0x6D,  # magic number
    0x01, 0x00, 0x00, 0x00,  # version
    # Type section
    0x01, 0x07, 0x01, 0x60, 0x02, 0x7F, 0x7F, 0x01, 0x7F,
    # Function section
    0x03, 0x02, 0x01, 0x00,
    # Export section
    0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
    # Code section
    0x0A, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6A, 0x0B
])


@pytest.fixture
def add_wasm():
    """Provide the simple add.wasm binary"""
    return ADD_WASM


@pytest.fixture
def test_wasm_dir():
    """Provide the test wasm directory path"""
    return Path(__file__).parent / "wasm"
