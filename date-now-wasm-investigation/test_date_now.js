/**
 * Test Date.now() behavior in mquickjs WASM
 */

const fs = require('fs');
const path = require('path');
const createMQuickJS = require('../mquickjs-sandbox/mquickjs.js');

async function runTests() {
    console.log("Testing Date.now() in mquickjs WASM...\n");

    // Load WASM binary manually for Node.js
    const wasmPath = path.join(__dirname, '../mquickjs-sandbox/mquickjs.wasm');
    const wasmBinary = fs.readFileSync(wasmPath);

    const Module = await createMQuickJS({ wasmBinary: wasmBinary });

    const sandbox_init = Module.cwrap('sandbox_init', 'number', ['number']);
    const sandbox_free = Module.cwrap('sandbox_free', null, []);
    const sandbox_eval = Module.cwrap('sandbox_eval', 'string', ['string']);
    const sandbox_get_error = Module.cwrap('sandbox_get_error', 'string', []);

    sandbox_init(1024 * 1024);

    // Test Date.now()
    console.log("Testing Date.now():");
    const result1 = sandbox_eval("Date.now()");
    console.log(`  First call: ${result1}`);

    const result2 = sandbox_eval("Date.now()");
    console.log(`  Second call: ${result2}`);

    const result3 = sandbox_eval("Date.now()");
    console.log(`  Third call: ${result3}`);

    // Test Date constructor
    console.log("\nTesting new Date():");
    const dateResult = sandbox_eval("new Date()");
    const dateError = sandbox_get_error();
    if (dateResult === null) {
        console.log(`  Error: ${dateError}`);
    } else {
        console.log(`  Result: ${dateResult}`);
    }

    // Test performance.now()
    console.log("\nTesting performance.now():");
    const perfResult = sandbox_eval("performance.now()");
    console.log(`  Result: ${perfResult}`);

    // Compare with native JS
    console.log("\nNative JS Date.now() for comparison:");
    console.log(`  ${Date.now()}`);

    sandbox_free();
}

runTests().catch(console.error);
