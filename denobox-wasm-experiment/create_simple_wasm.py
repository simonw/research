#!/usr/bin/env python3
"""
Create a simple WASM file directly from binary format.
This is a minimal WASM that exports an 'add' function.
"""

import struct

# WASM magic number and version
WASM_MAGIC = b'\x00asm'
WASM_VERSION = struct.pack('<I', 1)

def make_leb128_unsigned(value):
    """Encode an unsigned integer as LEB128."""
    result = []
    while True:
        byte = value & 0x7f
        value >>= 7
        if value:
            result.append(byte | 0x80)
        else:
            result.append(byte)
            break
    return bytes(result)

def make_section(section_id, content):
    """Create a WASM section."""
    size = make_leb128_unsigned(len(content))
    return bytes([section_id]) + size + content

# Section 1: Type section
# Define function type: (i32, i32) -> i32
type_section = bytes([
    1,           # number of types
    0x60,        # func type
    2, 0x7f, 0x7f,   # 2 params: i32, i32
    1, 0x7f,     # 1 result: i32
])

# Section 3: Function section
# Map function 0 to type 0
func_section = bytes([
    1,           # number of functions
    0,           # type index 0
])

# Section 7: Export section
# Export function 0 as "add"
export_name = b'add'
export_section = bytes([
    1,           # number of exports
    len(export_name),
]) + export_name + bytes([
    0x00,        # export kind: function
    0,           # function index 0
])

# Section 10: Code section
# Function body: local.get 0, local.get 1, i32.add
code_body = bytes([
    0,           # number of local declarations
    0x20, 0x00,  # local.get 0
    0x20, 0x01,  # local.get 1
    0x6a,        # i32.add
    0x0b,        # end
])
code_section = bytes([
    1,           # number of code entries
    len(code_body),
]) + code_body

# Assemble the WASM module
wasm = (
    WASM_MAGIC +
    WASM_VERSION +
    make_section(1, type_section) +  # Type section
    make_section(3, func_section) +  # Function section
    make_section(7, export_section) +  # Export section
    make_section(10, code_section)  # Code section
)

# Write to file
with open('/tmp/simple_add.wasm', 'wb') as f:
    f.write(wasm)

print(f"Created simple_add.wasm ({len(wasm)} bytes)")
print("This WASM exports an 'add(i32, i32) -> i32' function")
