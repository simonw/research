// Deno worker script for PyodideBox - uses pre-cached npm:pyodide
// Handles NDJSON communication over stdin/stdout with Pyodide Python runtime
// This version uses the Deno-cached npm:pyodide, no network needed after caching

let pyodide = null;
let pyodideReady = false;
let localPackagePath = null;

async function initPyodide() {
    if (pyodideReady) return;

    // Import from npm cache (no network needed if already cached)
    const { loadPyodide } = await import("npm:pyodide@0.27.5");

    const options = {};

    // If a local package path is set, use it for package loading
    if (localPackagePath) {
        options.packageCacheDir = localPackagePath;
    }

    pyodide = await loadPyodide(options);
    pyodideReady = true;
}

async function handleRequest(request) {
    const { id, type } = request;

    try {
        switch (type) {
            case "set_package_path": {
                // Optional: set a local path for package caching
                localPackagePath = request.path;
                return { id, result: { path: localPackagePath } };
            }

            case "init": {
                await initPyodide();
                const version = pyodide.version;
                return { id, result: { initialized: true, version } };
            }

            case "run_python": {
                if (!pyodideReady) {
                    await initPyodide();
                }
                const { code } = request;
                const result = await pyodide.runPythonAsync(code);
                let jsResult;
                if (result === undefined || result === null) {
                    jsResult = null;
                } else if (typeof result.toJs === "function") {
                    jsResult = result.toJs({ dict_converter: Object.fromEntries });
                } else {
                    jsResult = result;
                }
                return { id, result: jsResult };
            }

            case "set_global": {
                if (!pyodideReady) {
                    await initPyodide();
                }
                const { name, value } = request;
                pyodide.globals.set(name, value);
                return { id, result: true };
            }

            case "get_global": {
                if (!pyodideReady) {
                    await initPyodide();
                }
                const { name } = request;
                const value = pyodide.globals.get(name);
                let jsValue;
                if (value === undefined || value === null) {
                    jsValue = null;
                } else if (typeof value.toJs === "function") {
                    jsValue = value.toJs({ dict_converter: Object.fromEntries });
                } else {
                    jsValue = value;
                }
                return { id, result: jsValue };
            }

            case "install_packages": {
                if (!pyodideReady) {
                    await initPyodide();
                }
                const { packages } = request;
                await pyodide.loadPackage(packages);
                return { id, result: { installed: packages } };
            }

            case "run_js": {
                const { code } = request;
                const evalFunc = eval;
                let result = evalFunc(code);
                if (result instanceof Promise) {
                    result = await result;
                }
                return { id, result: result === undefined ? null : result };
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

    const reader = Deno.stdin.readable.getReader();
    let buffer = "";

    while (true) {
        const { done, value } = await reader.read();

        if (done) {
            break;
        }

        buffer += decoder.decode(value, { stream: true });

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
