#!/usr/bin/env python3
"""Test loading WASM modules with Denobox."""

from denobox import Denobox, DenoboxError

print("=== Testing WASM Loading with Denobox ===\n")

# Test loading MicroQuickJS WASM
wasm_path = "/tmp/tools/mquickjs_optimized.wasm"

print(f"Loading WASM from: {wasm_path}")

with Denobox() as box:
    try:
        wasm = box.load_wasm(path=wasm_path)
        print(f"Module ID: {wasm.module_id}")
        print(f"Exports: {wasm.exports}")
        print()

        # Check what functions are available
        print("Exported functions:")
        for name, func_type in wasm.exports.items():
            print(f"  - {name}: {func_type}")
        print()

    except DenoboxError as e:
        print(f"Error loading WASM: {e}")
    except Exception as e:
        print(f"Unexpected error: {type(e).__name__}: {e}")

print("\n=== Test completed ===")
