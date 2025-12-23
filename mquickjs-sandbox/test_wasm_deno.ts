/**
 * Test mquickjs WASM with Deno
 *
 * Run with: deno run --allow-read test_wasm_deno.ts
 */

// Load WASM binary manually
const wasmPath = new URL("./mquickjs.wasm", import.meta.url);
const wasmBinary = await Deno.readFile(wasmPath);

// Load and instantiate the emscripten module
// We need to create a minimal environment for emscripten

interface EmscriptenModule {
    cwrap: (name: string, returnType: string | null, argTypes: string[]) => (...args: unknown[]) => unknown;
    UTF8ToString: (ptr: number) => string;
    stringToUTF8: (str: string, ptr: number, maxBytes: number) => void;
    _malloc: (size: number) => number;
    _free: (ptr: number) => void;
}

// Import the emscripten JS as a module
const createModuleJs = await Deno.readTextFile(new URL("./mquickjs.js", import.meta.url));

// Create module factory function with Deno globals
const moduleCode = `
    var Module;
    var process = { exit: (code) => { if (code !== 0) throw new Error('Exit: ' + code); } };
    var require = () => ({});

    ${createModuleJs}

    createMQuickJS;
`;

const createMQuickJS = eval(moduleCode) as (options?: {wasmBinary?: Uint8Array}) => Promise<EmscriptenModule>;

console.log("Testing mquickjs WASM with Deno...");

const Module = await createMQuickJS({ wasmBinary: wasmBinary });

// Get function wrappers
const sandbox_init = Module.cwrap('sandbox_init', 'number', ['number']) as (memSize: number) => number;
const sandbox_free = Module.cwrap('sandbox_free', null, []) as () => void;
const sandbox_eval = Module.cwrap('sandbox_eval', 'string', ['string']) as (code: string) => string | null;
const sandbox_get_error = Module.cwrap('sandbox_get_error', 'string', []) as () => string;

// Initialize with 1MB memory
const result = sandbox_init(1024 * 1024);
if (!result) {
    throw new Error("Failed to initialize sandbox");
}
console.log("  Sandbox initialized");

// Run tests
const tests = [
    { code: "1 + 2", expected: "3" },
    { code: "'hello' + ' world'", expected: "hello world" },
    { code: "true && false", expected: "false" },
    { code: "Math.abs(-5)", expected: "5" },
    { code: "[1, 2, 3].length", expected: "3" },
];

let passed = 0;
let failed = 0;

for (const test of tests) {
    const evalResult = sandbox_eval(test.code);
    if (evalResult === null) {
        console.log(`  FAIL: ${test.code} -> error: ${sandbox_get_error()}`);
        failed++;
    } else if (evalResult === test.expected) {
        console.log(`  PASS: ${test.code} = ${evalResult}`);
        passed++;
    } else {
        console.log(`  FAIL: ${test.code} expected ${test.expected}, got ${evalResult}`);
        failed++;
    }
}

// Cleanup
sandbox_free();

console.log(`\nResults: ${passed} passed, ${failed} failed`);
if (failed > 0) {
    Deno.exit(1);
}
