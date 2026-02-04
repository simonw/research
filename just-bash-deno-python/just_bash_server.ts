#!/usr/bin/env -S deno run --allow-all
/**
 * just-bash JSONL Server
 *
 * A long-running process that accepts newline-delimited JSON commands on stdin
 * and returns results as newline-delimited JSON on stdout.
 *
 * Each Bash instance persists across requests, so filesystem state (virtual files,
 * directories, etc.) is maintained between commands.
 *
 * Input format (one JSON object per line):
 *   {"id": "uuid", "command": "echo hello"}
 *   {"id": "uuid", "command": "cat /tmp/file.txt", "env": {"KEY": "val"}, "cwd": "/tmp"}
 *
 * Output format (one JSON object per line):
 *   {"id": "uuid", "stdout": "hello\n", "stderr": "", "exit_code": 0}
 *   {"id": "uuid", "stdout": "", "stderr": "error msg", "exit_code": 1, "error": "..."}
 *
 * Special commands:
 *   {"id": "uuid", "command": "__ping"}       -> {"id": "uuid", "stdout": "pong", ...}
 *   {"id": "uuid", "command": "__shutdown"}    -> graceful shutdown
 *   {"id": "uuid", "command": "__reset"}       -> create fresh Bash instance
 *   {"id": "uuid", "command": "__list_files", "cwd": "/tmp"} -> list files in dir
 */

import { Bash } from "npm:just-bash";

// Parse CLI flags
const enableNetwork = Deno.args.includes("--network");

interface Request {
  id: string;
  command: string;
  env?: Record<string, string>;
  cwd?: string;
  timeout_ms?: number;
}

interface Response {
  id: string;
  stdout: string;
  stderr: string;
  exit_code: number;
  error?: string;
}

function createBash(): Bash {
  const opts: Record<string, unknown> = {};
  if (enableNetwork) {
    opts.network = { dangerouslyAllowFullInternetAccess: true };
  }
  return new Bash(opts);
}

let bash = createBash();

function sendResponse(resp: Response): void {
  const line = JSON.stringify(resp);
  // Write to stdout as a single line
  const encoder = new TextEncoder();
  Deno.stdout.writeSync(encoder.encode(line + "\n"));
}

async function handleRequest(req: Request): Promise<Response> {
  const { id, command } = req;

  // Special commands
  if (command === "__ping") {
    return { id, stdout: "pong", stderr: "", exit_code: 0 };
  }

  if (command === "__shutdown") {
    sendResponse({ id, stdout: "shutting down", stderr: "", exit_code: 0 });
    Deno.exit(0);
  }

  if (command === "__reset") {
    bash = createBash();
    return { id, stdout: "reset", stderr: "", exit_code: 0 };
  }

  if (command === "__list_files") {
    try {
      const dir = req.cwd || "/";
      const result = await bash.exec(`find ${dir} -maxdepth 1 -type f`);
      return { id, stdout: result.stdout, stderr: result.stderr, exit_code: result.exitCode };
    } catch (e) {
      return { id, stdout: "", stderr: String(e), exit_code: 1, error: String(e) };
    }
  }

  // Regular bash command execution
  try {
    const execOpts: Record<string, unknown> = {};
    if (req.env) execOpts.env = req.env;
    if (req.cwd) execOpts.cwd = req.cwd;

    let result;
    if (req.timeout_ms) {
      // Implement timeout via Promise.race
      const execPromise = bash.exec(command, execOpts);
      const timeoutPromise = new Promise<never>((_, reject) => {
        setTimeout(() => reject(new Error(`Timeout after ${req.timeout_ms}ms`)), req.timeout_ms);
      });
      result = await Promise.race([execPromise, timeoutPromise]);
    } else {
      result = await bash.exec(command, execOpts);
    }

    return {
      id,
      stdout: result.stdout,
      stderr: result.stderr,
      exit_code: result.exitCode,
    };
  } catch (e) {
    return {
      id,
      stdout: "",
      stderr: String(e),
      exit_code: 1,
      error: String(e),
    };
  }
}

// Signal readiness on stderr so clients know we're ready
Deno.stderr.writeSync(new TextEncoder().encode("READY\n"));

// Read stdin line by line
const decoder = new TextDecoder();
let buffer = "";

for await (const chunk of Deno.stdin.readable) {
  buffer += decoder.decode(chunk, { stream: true });

  // Process complete lines
  let newlineIdx: number;
  while ((newlineIdx = buffer.indexOf("\n")) !== -1) {
    const line = buffer.slice(0, newlineIdx).trim();
    buffer = buffer.slice(newlineIdx + 1);

    if (!line) continue;

    let req: Request;
    try {
      req = JSON.parse(line);
    } catch {
      // Invalid JSON - send error with empty id
      sendResponse({
        id: "",
        stdout: "",
        stderr: `Invalid JSON: ${line}`,
        exit_code: 1,
        error: "Invalid JSON input",
      });
      continue;
    }

    if (!req.id || !req.command) {
      sendResponse({
        id: req.id || "",
        stdout: "",
        stderr: "Missing required fields: id, command",
        exit_code: 1,
        error: "Missing required fields",
      });
      continue;
    }

    const resp = await handleRequest(req);
    sendResponse(resp);
  }
}
