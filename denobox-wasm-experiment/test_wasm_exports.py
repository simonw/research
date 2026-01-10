#!/usr/bin/env python3
"""Test loading downloaded WASM files with Denobox to see exports."""

from denobox import Denobox, DenoboxError

wasm_files = [
    ("/tmp/quickjs.wasm", "QuickJS (from @jitl/quickjs-wasmfile-release-sync)"),
    ("/tmp/micropython.wasm", "MicroPython (from @micropython/micropython-webassembly-pyscript)"),
    ("/tmp/tools/mquickjs_optimized.wasm", "MicroQuickJS Optimized (from simonw/tools)"),
    ("/tmp/tools/mquickjs.wasm", "MicroQuickJS Original (from simonw/tools)"),
]

print("=== Testing WASM Files with Denobox ===\n")

with Denobox() as box:
    for wasm_path, name in wasm_files:
        print(f"### {name}")
        print(f"Path: {wasm_path}")
        try:
            wasm = box.load_wasm(path=wasm_path)
            print(f"Module ID: {wasm.module_id}")
            print(f"Exports ({len(wasm.exports)} total):")
            for export_name, export_type in list(wasm.exports.items())[:20]:
                print(f"  - {export_name}: {export_type}")
            if len(wasm.exports) > 20:
                print(f"  ... and {len(wasm.exports) - 20} more")
            wasm.unload()
        except DenoboxError as e:
            print(f"Error: {e}")
        except Exception as e:
            print(f"Error: {type(e).__name__}: {e}")
        print()

print("=== Test completed ===")
