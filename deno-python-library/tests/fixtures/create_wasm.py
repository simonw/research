"""Create a simple WASM module for testing."""

# This is a minimal WASM module that exports an 'add' function
# The WAT source is:
# (module
#   (func (export "add") (param i32 i32) (result i32)
#     local.get 0
#     local.get 1
#     i32.add)
#   (func (export "multiply") (param i32 i32) (result i32)
#     local.get 0
#     local.get 1
#     i32.mul)
# )

wasm_bytes = bytes(
    [
        0x00,
        0x61,
        0x73,
        0x6D,  # Magic number
        0x01,
        0x00,
        0x00,
        0x00,  # Version 1
        # Type section
        0x01,  # Section ID: Type
        0x07,  # Section size: 7 bytes
        0x01,  # Number of types: 1
        0x60,  # func type
        0x02,  # Number of params: 2
        0x7F,  # i32
        0x7F,  # i32
        0x01,  # Number of results: 1
        0x7F,  # i32
        # Function section
        0x03,  # Section ID: Function
        0x03,  # Section size: 3 bytes
        0x02,  # Number of functions: 2
        0x00,  # Function 0 uses type 0
        0x00,  # Function 1 uses type 0
        # Export section
        0x07,  # Section ID: Export
        0x12,  # Section size: 18 bytes (1 + 6 + 11)
        0x02,  # Number of exports: 2
        0x03,  # Name length: 3
        0x61,
        0x64,
        0x64,  # "add"
        0x00,  # Export kind: function
        0x00,  # Function index: 0
        0x08,  # Name length: 8
        0x6D,
        0x75,
        0x6C,
        0x74,
        0x69,
        0x70,
        0x6C,
        0x79,  # "multiply"
        0x00,  # Export kind: function
        0x01,  # Function index: 1
        # Code section
        0x0A,  # Section ID: Code
        0x11,  # Section size: 17 bytes (1 + 8 + 8)
        0x02,  # Number of function bodies: 2
        # Function body 0 (add)
        0x07,  # Body size: 7 bytes
        0x00,  # Local variable count: 0
        0x20,
        0x00,  # local.get 0
        0x20,
        0x01,  # local.get 1
        0x6A,  # i32.add
        0x0B,  # end
        # Function body 1 (multiply)
        0x07,  # Body size: 7 bytes
        0x00,  # Local variable count: 0
        0x20,
        0x00,  # local.get 0
        0x20,
        0x01,  # local.get 1
        0x6C,  # i32.mul
        0x0B,  # end
    ]
)

if __name__ == "__main__":
    with open("tests/fixtures/math.wasm", "wb") as f:
        f.write(wasm_bytes)
    print(f"Created math.wasm ({len(wasm_bytes)} bytes)")
