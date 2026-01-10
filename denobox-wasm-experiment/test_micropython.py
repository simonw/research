#!/usr/bin/env python3
"""
Test MicroPython via Denobox using dynamic import.
"""

from denobox import Denobox, DenoboxError

print("=== Testing MicroPython via Denobox ===\n")

# JavaScript code to dynamically import and run MicroPython
# Deno can dynamically import from URLs
js_code = '''
(async () => {
    // Dynamic import of MicroPython ESM module
    const { loadMicroPython } = await import("https://cdn.jsdelivr.net/npm/@micropython/micropython-webassembly-pyscript@1.26.0/micropython.mjs");

    // Capture stdout
    let output = "";

    // Load MicroPython
    const mp = await loadMicroPython({
        heapsize: 1048576,  // 1MB heap
        stdout: (text) => { output += text + "\\n"; },
        stderr: (text) => { output += "ERROR: " + text + "\\n"; },
        linebuffer: true
    });

    // Run Python code
    const pythonCode = `
print("Hello from MicroPython!")
print(1 + 2 * 3)
import math
print(f"Pi = {math.pi:.4f}")
print([x**2 for x in range(5)])
`;

    await mp.runPythonAsync(pythonCode);

    return { success: true, output: output.trim() };
})()
'''

print("Executing MicroPython in Denobox via dynamic import...")

with Denobox() as box:
    try:
        result = box.eval(js_code)
        print(f"\nResult: {result}")
        if result.get('success'):
            print(f"\nMicroPython Output:")
            for line in result.get('output', '').split('\n'):
                print(f"  {line}")
            print("\nSuccess! MicroPython is running inside Denobox!")
    except DenoboxError as e:
        print(f"DenoboxError: {e}")
    except Exception as e:
        print(f"Error: {type(e).__name__}: {e}")

print("\n=== Test completed ===")
