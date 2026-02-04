/**
 * Test script for the just-bash JSONL server.
 * Spawns the server as a subprocess and sends commands via stdin/stdout.
 */

const encoder = new TextEncoder();
const decoder = new TextDecoder();

// Spawn the server
const command = new Deno.Command(Deno.execPath(), {
  args: ["run", "--allow-all", "just_bash_server.ts", "--network"],
  stdin: "piped",
  stdout: "piped",
  stderr: "piped",
  env: { ...Deno.env.toObject(), DENO_TLS_CA_STORE: "system" },
});

const process = command.spawn();

const writer = process.stdin.getWriter();
const reader = process.stdout.getReader();

// Read stderr for READY signal
const stderrReader = process.stderr.getReader();

async function waitForReady(): Promise<void> {
  let buf = "";
  while (true) {
    const { value, done } = await stderrReader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    if (buf.includes("READY")) return;
  }
  throw new Error("Server never became ready");
}

// Buffer for reading stdout lines
let stdoutBuffer = "";

async function readResponse(): Promise<Record<string, unknown>> {
  while (true) {
    const newlineIdx = stdoutBuffer.indexOf("\n");
    if (newlineIdx !== -1) {
      const line = stdoutBuffer.slice(0, newlineIdx);
      stdoutBuffer = stdoutBuffer.slice(newlineIdx + 1);
      return JSON.parse(line);
    }
    const { value, done } = await reader.read();
    if (done) throw new Error("Server stdout closed");
    stdoutBuffer += decoder.decode(value, { stream: true });
  }
}

async function sendCommand(
  id: string,
  command: string,
  opts?: Record<string, unknown>
): Promise<Record<string, unknown>> {
  const req = JSON.stringify({ id, command, ...opts }) + "\n";
  await writer.write(encoder.encode(req));
  return await readResponse();
}

// Utility
function uuid(): string {
  return crypto.randomUUID();
}

function assert(cond: boolean, msg: string): void {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    Deno.exit(1);
  }
}

// --- Tests ---
console.log("Waiting for server to be ready...");
await waitForReady();
console.log("Server is ready!\n");

let resp: Record<string, unknown>;
let passed = 0;
let failed = 0;

function check(name: string, cond: boolean, detail?: string): void {
  if (cond) {
    console.log(`  PASS: ${name}`);
    passed++;
  } else {
    console.log(`  FAIL: ${name}${detail ? " - " + detail : ""}`);
    failed++;
  }
}

// Test 1: Ping
console.log("Test 1: Ping");
resp = await sendCommand(uuid(), "__ping");
check("ping returns pong", (resp.stdout as string) === "pong");
check("exit code 0", resp.exit_code === 0);

// Test 2: Basic echo
console.log("\nTest 2: Basic echo");
const echoId = uuid();
resp = await sendCommand(echoId, "echo 'hello world'");
check("stdout correct", (resp.stdout as string).trim() === "hello world");
check("id matches", resp.id === echoId);
check("exit code 0", resp.exit_code === 0);

// Test 3: Write a file then read it (persistence test)
console.log("\nTest 3: Persistence - write then read file");
const writeId = uuid();
resp = await sendCommand(writeId, 'echo "persistent data" > /tmp/persist.txt');
check("write exit code 0", resp.exit_code === 0);

const readId = uuid();
resp = await sendCommand(readId, "cat /tmp/persist.txt");
check("read back correct", (resp.stdout as string).trim() === "persistent data");
check("read id matches", resp.id === readId);

// Test 4: Multi-command pipeline
console.log("\nTest 4: Pipeline");
resp = await sendCommand(uuid(), 'echo -e "cherry\\napple\\nbanana" | sort');
check("sorted output", (resp.stdout as string).trim() === "apple\nbanana\ncherry");

// Test 5: jq
console.log("\nTest 5: jq");
resp = await sendCommand(uuid(), 'echo \'{"name":"test","value":42}\' | jq .value');
check("jq extracts value", (resp.stdout as string).trim() === "42");

// Test 6: Error handling
console.log("\nTest 6: Error handling");
resp = await sendCommand(uuid(), "cat /nonexistent/file");
check("non-zero exit code", (resp.exit_code as number) !== 0);

// Test 7: Complex script
console.log("\nTest 7: Complex script");
resp = await sendCommand(
  uuid(),
  `
for i in 1 2 3 4 5; do
  echo "item $i"
done | grep "3"
`
);
check("complex script works", (resp.stdout as string).trim() === "item 3");

// Test 8: Virtual filesystem persistence across multiple operations
console.log("\nTest 8: Multi-step filesystem persistence");
await sendCommand(uuid(), "mkdir -p /workspace/project");
await sendCommand(uuid(), 'echo "line 1" > /workspace/project/data.txt');
await sendCommand(uuid(), 'echo "line 2" >> /workspace/project/data.txt');
await sendCommand(uuid(), 'echo "line 3" >> /workspace/project/data.txt');
resp = await sendCommand(uuid(), "wc -l < /workspace/project/data.txt");
check("3 lines written persistently", (resp.stdout as string).trim() === "3");

resp = await sendCommand(uuid(), "cat /workspace/project/data.txt");
check("content preserved", (resp.stdout as string).includes("line 1") && (resp.stdout as string).includes("line 3"));

// Test 9: Environment variables within a single command
console.log("\nTest 9: Env vars in single command");
resp = await sendCommand(uuid(), 'FOO=bar && echo "$FOO"');
check("env var works", (resp.stdout as string).trim() === "bar");

// Test 10: Per-request env override
console.log("\nTest 10: Per-request env override");
resp = await sendCommand(uuid(), 'echo "$CUSTOM_VAR"', { env: { CUSTOM_VAR: "hello_env" } });
check("custom env var", (resp.stdout as string).trim() === "hello_env");

// Test 11: sed
console.log("\nTest 11: sed");
await sendCommand(uuid(), 'echo "hello world" > /tmp/sed_test.txt');
resp = await sendCommand(uuid(), 'sed "s/world/deno/" /tmp/sed_test.txt');
check("sed substitution", (resp.stdout as string).trim() === "hello deno");

// Test 12: awk
console.log("\nTest 12: awk");
resp = await sendCommand(uuid(), "seq 1 10 | awk '{sum+=$1} END{print sum}'");
check("awk sum", (resp.stdout as string).trim() === "55");

// Test 13: curl (network enabled)
console.log("\nTest 13: curl");
resp = await sendCommand(uuid(), "curl -s https://httpbin.org/get | jq .url");
check("curl GET works", (resp.stdout as string).trim() === '"https://httpbin.org/get"');

// Test 14: Reset
console.log("\nTest 14: Reset");
await sendCommand(uuid(), 'echo "before reset" > /tmp/reset_test.txt');
resp = await sendCommand(uuid(), "cat /tmp/reset_test.txt");
check("file exists before reset", (resp.stdout as string).trim() === "before reset");

await sendCommand(uuid(), "__reset");
resp = await sendCommand(uuid(), "cat /tmp/reset_test.txt");
check("file gone after reset", (resp.exit_code as number) !== 0);

// Test 15: Timeout
console.log("\nTest 15: Timeout");
resp = await sendCommand(uuid(), "sleep 10", { timeout_ms: 500 });
check("timeout produces error", !!(resp.error as string));

// Shutdown
console.log("\n---");
console.log(`Results: ${passed} passed, ${failed} failed out of ${passed + failed} checks`);

await sendCommand(uuid(), "__shutdown");
await process.status;
