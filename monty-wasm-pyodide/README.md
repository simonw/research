# Monty WASM + Pyodide

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

[Monty](https://github.com/pydantic/monty) is a sandboxed Python interpreter written in Rust. This project compiles it to WebAssembly in two ways:

1. **Standalone WASM** — Use directly from JavaScript with zero dependencies
2. **Pyodide wheel** — Use as a Python package inside Pyodide

## Live demos

- **[Standalone WASM demo](https://simonw.github.io/research/monty-wasm-pyodide/demo.html)** — Run Python in your browser via the Monty WASM module
- **[Pyodide demo](https://simonw.github.io/research/monty-wasm-pyodide/pyodide-demo.html)** — Run Python via Pyodide with the pydantic_monty wheel

## Standalone WASM usage

Load `monty_wasm.js` as an ES module. Call `init()` to load the WASM binary, then use the `Monty` class.

### Basic usage

```html
<script type="module">
import init, { Monty } from 'https://simonw.github.io/research/monty-wasm-pyodide/monty_wasm.js';

// Initialize the WASM module
await init();

// Run a simple expression
const m = new Monty('1 + 2 * 3');
const result = m.run();       // Returns JSON string: "7"
console.log(JSON.parse(result)); // 7
</script>
```

### Input variables

```javascript
import init, { Monty } from './monty_wasm.js';
await init();

// Create interpreter with named input variables
const m = Monty.withInputs('x + y', '["x", "y"]');

// Run with specific input values (passed as JSON object)
const result = m.runWithInputs('{"x": 10, "y": 20}');
console.log(JSON.parse(result)); // 30

// Reuse with different values
const result2 = m.runWithInputs('{"x": 100, "y": 200}');
console.log(JSON.parse(result2)); // 300
```

### Capturing print output

```javascript
import init, { Monty } from './monty_wasm.js';
await init();

const m = new Monty('print("Hello from Monty!")\nprint("Line 2")');
const output = m.runCapturePrint();
console.log(output);
// "Hello from Monty!\nLine 2\n"
```

### Multiline code with loops

```javascript
import init, { Monty } from './monty_wasm.js';
await init();

const code = `
x = 0
for i in range(10):
    x = x + i
x
`;

const m = new Monty(code);
const result = m.run();
console.log(JSON.parse(result)); // 45
```

### Error handling

```javascript
import init, { Monty } from './monty_wasm.js';
await init();

try {
  const m = new Monty('1 / 0');
  m.run();
} catch (e) {
  console.log(e.message); // "ZeroDivisionError: division by zero"
}
```

### Return types

The `run()` method returns a JSON string. Parse it to get JavaScript values:

```javascript
JSON.parse(new Monty('42').run())              // 42 (number)
JSON.parse(new Monty('"hello"').run())          // "hello" (string)
JSON.parse(new Monty('[1, 2, 3]').run())        // [1, 2, 3] (array)
JSON.parse(new Monty('{"a": 1}').run())         // {a: 1} (object)
JSON.parse(new Monty('True').run())             // true (boolean)
JSON.parse(new Monty('None').run())             // null
```

### Complete HTML example

```html
<!DOCTYPE html>
<html>
<head><title>Monty WASM Example</title></head>
<body>
  <textarea id="code" rows="10" cols="60">
for i in range(5):
    print("*" * (i + 1))
  </textarea>
  <button id="run">Run</button>
  <pre id="output"></pre>

  <script type="module">
    import init, { Monty } from 'https://simonw.github.io/research/monty-wasm-pyodide/monty_wasm.js';
    await init();

    document.getElementById('run').addEventListener('click', () => {
      const code = document.getElementById('code').value;
      try {
        const m = new Monty(code);
        const printOutput = m.runCapturePrint();
        document.getElementById('output').textContent = printOutput;
      } catch (e) {
        document.getElementById('output').textContent = 'Error: ' + e.message;
      }
    });
  </script>
</body>
</html>
```

## Pyodide wheel usage

Install the wheel in Pyodide using micropip, then use the `pydantic_monty` Python API.

### Basic usage

```html
<script src="https://cdn.jsdelivr.net/pyodide/v0.29.3/full/pyodide.js"></script>
<script>
async function main() {
  const pyodide = await loadPyodide();
  await pyodide.loadPackage('micropip');
  const micropip = pyodide.pyimport('micropip');
  await micropip.install(
    'https://simonw.github.io/research/monty-wasm-pyodide/pydantic_monty-0.0.3-cp313-cp313-emscripten_4_0_9_wasm32.whl'
  );

  // Basic arithmetic
  const result = pyodide.runPython(`
    from pydantic_monty import Monty
    m = Monty('1 + 2 * 3')
    m.run()
  `);
  console.log(result); // 7
}
main();
</script>
```

### Input variables

```javascript
const result = pyodide.runPython(`
  from pydantic_monty import Monty
  m = Monty('x + y', inputs=['x', 'y'])
  m.run(inputs={"x": 10, "y": 20})
`);
console.log(result); // 30
```

### Error handling

```javascript
const result = pyodide.runPython(`
  from pydantic_monty import Monty, MontyRuntimeError
  m = Monty('1 / 0')
  try:
      m.run()
  except MontyRuntimeError as e:
      str(e)
`);
console.log(result); // "ZeroDivisionError: division by zero"
```

### Multiline code

```javascript
const result = pyodide.runPython(`
  from pydantic_monty import Monty
  code = '''
  x = 0
  for i in range(10):
      x = x + i
  x
  '''
  m = Monty(code)
  m.run()
`);
console.log(result); // 45
```

## API reference

### Standalone WASM (`Monty` class)

| Method | Description |
|--------|-------------|
| `new Monty(code)` | Create interpreter with Python code |
| `Monty.withInputs(code, inputsJson)` | Create with named input variables (JSON array of names) |
| `m.run()` | Run code, return result as JSON string |
| `m.runWithInputs(valuesJson)` | Run with input values (JSON object) |
| `m.runCapturePrint()` | Run code, return captured print output as string |
| `Monty.version()` | Get interpreter version string |

### Pyodide wheel (`pydantic_monty` module)

| API | Description |
|-----|-------------|
| `Monty(code)` | Create interpreter |
| `Monty(code, inputs=['x', 'y'])` | Create with named inputs |
| `m.run()` | Run and return result |
| `m.run(inputs={"x": 1, "y": 2})` | Run with input values |
| `MontyRuntimeError` | Exception class for runtime errors |

## Building from source

### Pyodide wheel

```bash
./build.sh
```

See `build.sh` for full details. Requires emsdk 4.0.9, Rust nightly-2025-06-27, and maturin.

### Standalone WASM

The standalone WASM build requires adding a `monty-wasm` crate to the monty workspace. See `monty-wasm-crate.diff` for the changes made to the monty repo.

```bash
cd /tmp/monty
# Apply the diff (adds crates/monty-wasm)
# Then build:
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
  wasm-pack build crates/monty-wasm --target web --release
```

## Running tests

```bash
npm install
npx playwright install firefox
npm run setup    # Download Pyodide runtime for local testing
HOME=/root npm test
```

Tests use Firefox and a local Node.js HTTP server (`serve.js`) with correct MIME types for ES modules and WASM files.

## Files

| File | Description |
|------|-------------|
| `demo.html` | Standalone WASM demo page |
| `pyodide-demo.html` | Pyodide + wheel demo page |
| `monty_wasm.js` | Standalone WASM JavaScript bindings (ES module) |
| `monty_wasm_bg.wasm` | Standalone WASM binary (2.8 MB) |
| `pydantic_monty-*.whl` | Pyodide wheel (4.0 MB) |
| `build.sh` | Pyodide wheel build script |
| `monty-wasm-crate.diff` | Diff to add monty-wasm crate to monty repo |
| `serve.js` | Node.js dev server with correct MIME types |
| `tests/monty-wasm.spec.js` | Playwright tests for standalone WASM (7 tests) |
| `tests/monty-pyodide.spec.js` | Playwright tests for Pyodide wheel (8 tests) |
