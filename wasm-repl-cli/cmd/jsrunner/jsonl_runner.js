// JSONL runner for QuickJS WASI - maintains persistent state across requests

// WASI doesn't have std module, use scriptArgs and print instead
// Read stdin line by line using the WASI-compatible approach

// Buffer for reading stdin
let stdinBuffer = '';
let stdinEOF = false;

function readLine() {
    // QuickJS WASI uses a different approach for stdin
    // We need to read character by character until newline
    while (true) {
        const newlinePos = stdinBuffer.indexOf('\n');
        if (newlinePos !== -1) {
            const line = stdinBuffer.substring(0, newlinePos);
            stdinBuffer = stdinBuffer.substring(newlinePos + 1);
            return line;
        }

        if (stdinEOF) {
            if (stdinBuffer.length > 0) {
                const line = stdinBuffer;
                stdinBuffer = '';
                return line;
            }
            return null;
        }

        // Use std.loadFile or read from file descriptor 0
        // In WASI-only mode, we need a different approach
        // QuickJS provides os.read for WASI
        try {
            // Try to read more data (this might not work in all QuickJS WASI builds)
            const chunk = std.in.getline();
            if (chunk === null) {
                stdinEOF = true;
            } else {
                stdinBuffer += chunk + '\n';
            }
        } catch (e) {
            stdinEOF = true;
        }
    }
}

// Main processing loop
let line;
while ((line = std.in.getline()) !== null) {
    const trimmed = line.trim();
    if (trimmed === '') {
        continue;
    }

    let request;
    try {
        request = JSON.parse(trimmed);
    } catch (e) {
        const response = { id: '', error: `Invalid JSON: ${e.message}` };
        print(JSON.stringify(response));
        continue;
    }

    const reqId = request.id || '';
    const code = request.code || '';

    let output = '';
    let error = null;

    // Capture console.log and print output
    const oldLog = console.log;
    const oldPrint = globalThis.print;

    const captureOutput = function(...args) {
        output += args.map(a => String(a)).join(' ') + '\n';
    };

    console.log = captureOutput;
    globalThis.print = captureOutput;

    try {
        // Try to evaluate as expression first
        const result = eval(code);
        if (result !== undefined) {
            output += String(result) + '\n';
        }
    } catch (e) {
        error = e.message || String(e);
    }

    console.log = oldLog;
    globalThis.print = oldPrint;

    const response = { id: reqId };
    if (output) {
        response.output = output;
    }
    if (error) {
        response.error = error;
    }

    print(JSON.stringify(response));
}
