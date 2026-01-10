// Deno worker script for denobox
// Handles NDJSON communication over stdin/stdout

const wasmModules = new Map();
let moduleCounter = 0;

async function handleRequest(request) {
    const { id, type } = request;

    try {
        switch (type) {
            case "eval": {
                const { code } = request;
                // Use indirect eval to execute in global scope
                const evalFunc = eval;
                let result = evalFunc(code);
                // Handle promises
                if (result instanceof Promise) {
                    result = await result;
                }
                return { id, result: result === undefined ? null : result };
            }

            case "load_wasm": {
                const { bytes } = request;
                if (!bytes) {
                    throw new Error("'bytes' (base64-encoded WASM) must be provided");
                }

                // Decode base64 to Uint8Array
                const binaryString = atob(bytes);
                const wasmSource = new Uint8Array(binaryString.length);
                for (let i = 0; i < binaryString.length; i++) {
                    wasmSource[i] = binaryString.charCodeAt(i);
                }

                const wasmModule = await WebAssembly.compile(wasmSource);
                const instance = await WebAssembly.instantiate(wasmModule, {});

                const moduleId = `wasm_${moduleCounter++}`;
                const exports = {};

                for (const [name, value] of Object.entries(instance.exports)) {
                    if (typeof value === "function") {
                        exports[name] = "function";
                    } else if (value instanceof WebAssembly.Memory) {
                        exports[name] = "memory";
                    } else if (value instanceof WebAssembly.Table) {
                        exports[name] = "table";
                    } else if (value instanceof WebAssembly.Global) {
                        exports[name] = "global";
                    }
                }

                wasmModules.set(moduleId, instance);
                return { id, result: { moduleId, exports } };
            }

            case "call_wasm": {
                const { moduleId, func, args } = request;
                const instance = wasmModules.get(moduleId);
                if (!instance) {
                    throw new Error(`WASM module not found: ${moduleId}`);
                }

                const fn = instance.exports[func];
                if (typeof fn !== "function") {
                    throw new Error(`Export '${func}' is not a function`);
                }

                const result = fn(...(args || []));
                return { id, result: result === undefined ? null : result };
            }

            case "unload_wasm": {
                const { moduleId } = request;
                wasmModules.delete(moduleId);
                return { id, result: true };
            }

            case "shutdown": {
                return { id, result: true, shutdown: true };
            }

            default:
                throw new Error(`Unknown request type: ${type}`);
        }
    } catch (error) {
        return { id, error: error.message, stack: error.stack };
    }
}

async function main() {
    const decoder = new TextDecoder();
    const encoder = new TextEncoder();

    // Read from stdin line by line
    const reader = Deno.stdin.readable.getReader();
    let buffer = "";

    while (true) {
        const { done, value } = await reader.read();

        if (done) {
            break;
        }

        buffer += decoder.decode(value, { stream: true });

        // Process complete lines
        let newlineIndex;
        while ((newlineIndex = buffer.indexOf("\n")) !== -1) {
            const line = buffer.slice(0, newlineIndex);
            buffer = buffer.slice(newlineIndex + 1);

            if (line.trim() === "") continue;

            try {
                const request = JSON.parse(line);
                const response = await handleRequest(request);
                const output = JSON.stringify(response) + "\n";
                await Deno.stdout.write(encoder.encode(output));

                if (response.shutdown) {
                    reader.releaseLock();
                    return;
                }
            } catch (error) {
                const errorResponse = { error: `Parse error: ${error.message}` };
                await Deno.stdout.write(encoder.encode(JSON.stringify(errorResponse) + "\n"));
            }
        }
    }
}

main();
