// Deno worker script for v86box
// Handles NDJSON communication over stdin/stdout
// Boots a v86 Linux VM and executes bash commands

// Get v86 library path from environment or use npm
const v86LibPath = Deno.env.get("V86_LIB_PATH");
let V86;
if (v86LibPath) {
    const module = await import(v86LibPath);
    V86 = module.V86;
} else {
    // Try npm import (may fail in some environments)
    const module = await import("npm:v86@0.5.301");
    V86 = module.V86;
}

// Configuration - paths to required files
const CONFIG = {
    biosPath: null,      // Will be set from environment or embedded
    vgaBiosPath: null,
    bzImagePath: null,
    wasmPath: null,
    memorySize: 64 * 1024 * 1024,  // 64 MB
    vgaMemorySize: 2 * 1024 * 1024, // 2 MB
};

// Shell prompt pattern for this Buildroot image
const PROMPT_PATTERN = /% $/;

// State
let emulator = null;
let isReady = false;
let outputBuffer = "";
let currentCommand = null;
let commandOutput = "";
let outputTruncated = false;
const MAX_OUTPUT_SIZE = 1024 * 1024; // 1 MB max output

// Pending requests queue
const pendingRequests = new Map();

// Read NDJSON from stdin and write to stdout
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function sendResponse(response) {
    const line = JSON.stringify(response) + "\n";
    Deno.stdout.writeSync(encoder.encode(line));
}

function debug(msg) {
    // Uncomment for debugging
    // console.error(`[DEBUG] ${msg}`);
}

async function initEmulator() {
    debug("Loading files...");

    // Get paths from environment or use defaults
    const biosPath = Deno.env.get("V86_BIOS_PATH") || CONFIG.biosPath;
    const vgaBiosPath = Deno.env.get("V86_VGA_BIOS_PATH") || CONFIG.vgaBiosPath;
    const bzImagePath = Deno.env.get("V86_BZIMAGE_PATH") || CONFIG.bzImagePath;
    const wasmPath = Deno.env.get("V86_WASM_PATH") || CONFIG.wasmPath;

    if (!biosPath || !vgaBiosPath || !bzImagePath || !wasmPath) {
        throw new Error("Missing required file paths. Set V86_BIOS_PATH, V86_VGA_BIOS_PATH, V86_BZIMAGE_PATH, V86_WASM_PATH environment variables.");
    }

    // Load and compile WASM
    const wasmData = await Deno.readFile(wasmPath);
    const wasmModule = await WebAssembly.compile(wasmData);
    debug("WASM compiled");

    // Create emulator
    emulator = new V86({
        wasm_fn: async (imports) => {
            const instance = await WebAssembly.instantiate(wasmModule, imports);
            return instance.exports;
        },
        memory_size: CONFIG.memorySize,
        vga_memory_size: CONFIG.vgaMemorySize,
        bios: { url: biosPath },
        vga_bios: { url: vgaBiosPath },
        bzimage: { url: bzImagePath },
        autostart: true,
        disable_keyboard: true,
        disable_mouse: true,
    });

    debug("Emulator created");

    // Handle serial output
    emulator.add_listener("serial0-output-byte", handleSerialOutput);
}

function handleSerialOutput(byte) {
    const char = String.fromCharCode(byte);
    if (char === '\r') return;

    outputBuffer += char;

    // Keep buffer manageable
    if (outputBuffer.length > 2000) {
        outputBuffer = outputBuffer.slice(-1000);
    }

    // Track command output if we're waiting for a command
    if (currentCommand !== null) {
        if (!outputTruncated) {
            commandOutput += char;
            if (commandOutput.length >= MAX_OUTPUT_SIZE) {
                outputTruncated = true;
            }
        }

        // Check if command completed (prompt appeared)
        if (PROMPT_PATTERN.test(outputBuffer)) {
            completeCommand();
        }
    }

    // Check for initial boot completion
    if (!isReady && PROMPT_PATTERN.test(outputBuffer)) {
        isReady = true;
        debug("VM ready");
        // Signal ready to Python
        sendResponse({ type: "ready" });
    }
}

function completeCommand() {
    if (currentCommand === null) return;

    const request = pendingRequests.get(currentCommand.id);
    if (!request) return;

    // Parse output: skip echoed command and trailing prompt
    let lines = commandOutput.split('\n');

    // Remove the echoed command (first line)
    if (lines.length > 0) {
        lines = lines.slice(1);
    }

    // Remove trailing prompt remnants
    while (lines.length > 0) {
        const last = lines[lines.length - 1];
        if (last === '' || PROMPT_PATTERN.test(last)) {
            lines.pop();
        } else {
            break;
        }
    }

    const result = lines.join('\n');

    sendResponse({
        id: currentCommand.id,
        result: result,
        truncated: outputTruncated,
    });

    pendingRequests.delete(currentCommand.id);
    currentCommand = null;
    commandOutput = "";
    outputTruncated = false;
    outputBuffer = "";

    // Process next queued command if any
    processNextCommand();
}

// Queue of commands waiting to be executed
const commandQueue = [];

function processNextCommand() {
    if (currentCommand !== null) return;
    if (commandQueue.length === 0) return;

    const request = commandQueue.shift();
    executeCommand(request);
}

function executeCommand(request) {
    currentCommand = { id: request.id };
    commandOutput = "";
    outputTruncated = false;
    outputBuffer = "";

    // Send command to VM
    emulator.serial0_send(request.command + "\n");
}

async function handleRequest(request) {
    const { id, type } = request;

    try {
        switch (type) {
            case "exec": {
                const { command } = request;

                if (!isReady) {
                    sendResponse({ id, error: "VM not ready yet" });
                    return;
                }

                // Queue the command
                pendingRequests.set(id, request);
                commandQueue.push({ id, command });

                // Process if no command is running
                if (currentCommand === null) {
                    processNextCommand();
                }

                // Response will be sent when command completes
                break;
            }

            case "status": {
                sendResponse({
                    id,
                    result: {
                        ready: isReady,
                        pending: pendingRequests.size,
                    }
                });
                break;
            }

            case "shutdown": {
                sendResponse({ id, result: true, shutdown: true });
                if (emulator) {
                    emulator.stop();
                }
                return true; // Signal to exit
            }

            default:
                sendResponse({ id, error: `Unknown request type: ${type}` });
        }
    } catch (error) {
        sendResponse({ id, error: error.message, stack: error.stack });
    }

    return false;
}

async function main() {
    debug("Worker starting...");

    // Initialize the emulator
    try {
        await initEmulator();
    } catch (error) {
        sendResponse({ error: `Failed to initialize: ${error.message}` });
        Deno.exit(1);
    }

    // Read from stdin line by line
    const reader = Deno.stdin.readable.getReader();
    let buffer = "";

    while (true) {
        const { done, value } = await reader.read();

        if (done) break;

        buffer += decoder.decode(value, { stream: true });

        // Process complete lines
        let newlineIndex;
        while ((newlineIndex = buffer.indexOf("\n")) !== -1) {
            const line = buffer.slice(0, newlineIndex);
            buffer = buffer.slice(newlineIndex + 1);

            if (line.trim() === "") continue;

            try {
                const request = JSON.parse(line);
                const shouldExit = await handleRequest(request);
                if (shouldExit) {
                    reader.releaseLock();
                    return;
                }
            } catch (error) {
                sendResponse({ error: `Parse error: ${error.message}` });
            }
        }
    }
}

main();
