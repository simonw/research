/**
 * Test mquickjs WASM with Node.js
 */

const fs = require('fs');
const path = require('path');
const createMQuickJS = require('./mquickjs.js');

async function runTests() {
    console.log("Testing mquickjs WASM with Node.js...");

    // Load WASM binary manually for Node.js (fetch doesn't work with file paths)
    const wasmPath = path.join(__dirname, 'mquickjs.wasm');
    const wasmBinary = fs.readFileSync(wasmPath);

    const Module = await createMQuickJS({ wasmBinary: wasmBinary });

    // Get function wrappers
    const sandbox_init = Module.cwrap('sandbox_init', 'number', ['number']);
    const sandbox_free = Module.cwrap('sandbox_free', null, []);
    const sandbox_eval = Module.cwrap('sandbox_eval', 'string', ['string']);
    const sandbox_get_error = Module.cwrap('sandbox_get_error', 'string', []);

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
        const result = sandbox_eval(test.code);
        if (result === null) {
            console.log(`  FAIL: ${test.code} -> error: ${sandbox_get_error()}`);
            failed++;
        } else if (result === test.expected) {
            console.log(`  PASS: ${test.code} = ${result}`);
            passed++;
        } else {
            console.log(`  FAIL: ${test.code} expected ${test.expected}, got ${result}`);
            failed++;
        }
    }

    // Cleanup
    sandbox_free();

    console.log(`\nResults: ${passed} passed, ${failed} failed`);
    process.exit(failed > 0 ? 1 : 0);
}

runTests().catch(err => {
    console.error("Error:", err);
    process.exit(1);
});
