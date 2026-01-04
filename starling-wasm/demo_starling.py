# /// script
# dependencies = [
#   "wasmtime",
# ]
# ///
"""
StarlingMonkey WASM JavaScript Interpreter Demo

This script demonstrates inspecting and running JavaScript code using the
StarlingMonkey WebAssembly runtime.

## Architecture Notes

StarlingMonkey provides two WASM binaries:

1. **starling-raw.wasm** - A core WebAssembly module that can be:
   - Specialized using wizer/componentize for specific JS applications
   - Used for ahead-of-time compilation of JS code
   - Requires complex WASI Preview 1 and Preview 2 imports

2. **starling.wasm** - A WebAssembly Component (WASI 0.2) that:
   - Can run arbitrary JavaScript at runtime
   - Requires a component-aware runtime like wasmtime CLI
   - Cannot be loaded as a regular WASM module

The wasmtime-py library currently supports core WASM modules but not the full
WASI 0.2 Component Model that starling.wasm requires. Therefore:
- We use wasmtime-py to inspect starling-raw.wasm (the core module)
- We use wasmtime CLI to execute JavaScript via starling.wasm (the component)
"""

import wasmtime
import subprocess
import sys
import os
from pathlib import Path


def get_script_dir():
    """Get the directory containing this script."""
    return Path(__file__).parent


def inspect_wasm_module(wasm_path: str):
    """
    Use wasmtime-py to inspect a WASM module and show its structure.

    This demonstrates using the wasmtime Python module directly.
    """
    engine = wasmtime.Engine()

    try:
        module = wasmtime.Module.from_file(engine, wasm_path)

        print(f"Module: {Path(wasm_path).name}")
        print(f"  Validated: Yes")

        # List exports
        exports = list(module.exports)
        print(f"  Exports ({len(exports)}):")
        for exp in exports[:10]:  # Show first 10
            type_name = type(exp.type).__name__.replace('Type', '')
            print(f"    - {exp.name}: {type_name}")
        if len(exports) > 10:
            print(f"    ... and {len(exports) - 10} more")

        # List imports (summary)
        imports = list(module.imports)
        print(f"  Imports ({len(imports)}):")

        # Group imports by module
        import_modules = {}
        for imp in imports:
            if imp.module not in import_modules:
                import_modules[imp.module] = []
            import_modules[imp.module].append(imp.name)

        for mod, names in sorted(import_modules.items()):
            print(f"    - {mod}: {len(names)} functions")

        return True

    except Exception as e:
        print(f"Failed to load module: {e}")
        return False


def run_js_via_wasmtime_cli(code: str, wasm_path: str = None) -> str:
    """
    Run JavaScript code using StarlingMonkey via wasmtime CLI.

    StarlingMonkey's starling.wasm is a WASI 0.2 Component which requires
    the full wasmtime CLI to execute. The -S http flag enables HTTP support.
    """
    if wasm_path is None:
        wasm_path = str(get_script_dir() / "starling.wasm")

    # Find wasmtime in PATH or common locations
    wasmtime_paths = [
        "wasmtime",
        os.path.expanduser("~/.wasmtime/bin/wasmtime"),
        "/usr/local/bin/wasmtime",
    ]

    wasmtime_cmd = None
    for path in wasmtime_paths:
        try:
            subprocess.run([path, "--version"], capture_output=True, check=True)
            wasmtime_cmd = path
            break
        except (subprocess.CalledProcessError, FileNotFoundError):
            continue

    if wasmtime_cmd is None:
        raise RuntimeError(
            "wasmtime CLI not found. Install from https://wasmtime.dev/"
        )

    # Execute JavaScript via starling.wasm component
    result = subprocess.run(
        [wasmtime_cmd, "-S", "http", wasm_path, "-e", code],
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        raise RuntimeError(f"JavaScript execution failed: {result.stderr}")

    return result.stdout


def demo_module_inspection():
    """Demonstrate using wasmtime-py to inspect the WASM module."""
    print("=" * 60)
    print("WASM Module Inspection using wasmtime-py")
    print("=" * 60)
    print()

    raw_path = get_script_dir() / "starling-raw.wasm"

    if raw_path.exists():
        inspect_wasm_module(str(raw_path))
        print()
        print("Note: This is the core module. The component (starling.wasm)")
        print("cannot be loaded by wasmtime-py as it uses the Component Model.")
    else:
        print(f"starling-raw.wasm not found at {raw_path}")
        print("Download it from: https://github.com/bytecodealliance/StarlingMonkey/releases")


def demo_js_execution():
    """Demonstrate running JavaScript via wasmtime CLI."""
    print()
    print("=" * 60)
    print("JavaScript Execution via wasmtime CLI")
    print("=" * 60)
    print()

    demos = [
        ("Hello World", "console.log('Hello from StarlingMonkey!');"),

        ("Fibonacci", """
function fibonacci(n) {
    const seq = [0, 1];
    for (let i = 2; i < n; i++) {
        seq.push(seq[i-1] + seq[i-2]);
    }
    return seq;
}
console.log('Fibonacci(15):', fibonacci(15).join(', '));
"""),

        ("JSON Processing", """
const data = {
    name: 'StarlingMonkey',
    engine: 'SpiderMonkey',
    features: ['WASI 0.2', 'Component Model', 'HTTP fetch']
};
console.log(JSON.stringify(data, null, 2));
"""),

        ("Array Operations", """
const nums = [1, 2, 3, 4, 5];
console.log('Original:', nums);
console.log('Doubled:', nums.map(n => n * 2));
console.log('Sum:', nums.reduce((a, b) => a + b, 0));
"""),

        ("Prime Numbers", """
function isPrime(n) {
    if (n < 2) return false;
    for (let i = 2; i <= Math.sqrt(n); i++) {
        if (n % i === 0) return false;
    }
    return true;
}
const primes = [];
for (let i = 2; i <= 50; i++) {
    if (isPrime(i)) primes.push(i);
}
console.log('Primes up to 50:', primes.join(', '));
"""),
    ]

    for name, code in demos:
        print(f"--- {name} ---")
        try:
            output = run_js_via_wasmtime_cli(code)
            print(output)
        except Exception as e:
            print(f"Error: {e}")
            return False

    return True


def main():
    """Run the StarlingMonkey demo."""
    print()
    print("StarlingMonkey WASM JavaScript Interpreter Demo")
    print("Using wasmtime-py for inspection, wasmtime CLI for execution")
    print()

    try:
        # Part 1: Module inspection using wasmtime-py
        demo_module_inspection()

        # Part 2: JavaScript execution
        success = demo_js_execution()

        print("=" * 60)
        if success:
            print("All demos completed successfully!")
        else:
            print("Some demos failed.")
        print("=" * 60)

        return 0 if success else 1

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
