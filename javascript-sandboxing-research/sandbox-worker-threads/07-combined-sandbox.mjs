// Combined sandbox: Worker thread + Permission Model + isolated-vm
// This demonstrates the strongest sandboxing available with worker_threads
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Combined Sandbox: Worker + Permission Model + isolated-vm ===\n');

  // Create a sandboxed worker with permission model restrictions
  const worker = new Worker(new URL(import.meta.url), {
    execArgv: [
      '--experimental-permission',
      // Only allow reading the worker file itself and node_modules
      '--allow-fs-read=' + new URL('.', import.meta.url).pathname + '*,' +
        process.execPath,  // Allow reading the node binary itself
      // No --allow-fs-write  → all writes blocked
      // No --allow-child-process → spawn/exec blocked
      // No --allow-worker → nested workers blocked
      // No --allow-net → (hypothetical, not yet in Node 22)
    ],
  });

  worker.on('message', (msg) => {
    if (msg.type === 'log') console.log('  [sandbox]', ...msg.args);
    else if (msg.type === 'result') console.log('  Result:', msg.value);
    else if (msg.type === 'error') console.log('  Error:', msg.value);
    else console.log('  [worker]', msg);
  });
  worker.on('error', (err) => console.log('Worker ERROR:', err.message));
  worker.on('exit', (code) => console.log('Worker exited:', code));

  // Send untrusted code to execute
  setTimeout(() => {
    worker.postMessage({
      type: 'execute',
      code: `
        // This code runs in an isolated-vm isolate, inside a permission-restricted worker
        const result = [];
        for (let i = 1; i <= 10; i++) {
          result.push(i * i);
        }
        log('Squares: ' + result.join(', '));

        // Try to escape
        try { require('fs'); } catch(e) { log('require: blocked'); }
        try { process.exit(); } catch(e) { log('process: blocked'); }

        JSON.stringify({ squares: result });
      `,
      timeout: 5000,
      memoryLimit: 32,
    });
  }, 500);

  // Test with a CPU-hogging script
  setTimeout(() => {
    worker.postMessage({
      type: 'execute',
      code: 'while(true) {}',
      timeout: 1000,
      memoryLimit: 16,
    });
  }, 2000);

  // Test with memory-hogging script
  setTimeout(() => {
    worker.postMessage({
      type: 'execute',
      code: 'const d = []; while(true) { d.push(Array.from({length:1000}, () => Math.random())); }',
      timeout: 10000,
      memoryLimit: 16,
    });
  }, 4000);

  // Graceful shutdown
  setTimeout(() => {
    worker.postMessage({ type: 'shutdown' });
  }, 8000);

} else {
  // Worker code: sets up isolated-vm sandbox executor
  const ivm = (await import('isolated-vm')).default;

  parentPort.on('message', async (msg) => {
    if (msg.type === 'shutdown') {
      process.exit(0);
    }

    if (msg.type === 'execute') {
      const { code, timeout = 5000, memoryLimit = 32 } = msg;

      let isolate;
      try {
        isolate = new ivm.Isolate({ memoryLimit });
        const context = await isolate.createContext();
        const jail = context.global;

        // Provide only safe APIs
        await jail.set('log', new ivm.Callback(
          (...args) => parentPort.postMessage({ type: 'log', args })
        ));

        const script = await isolate.compileScript(code);
        const result = await script.run(context, { timeout });
        parentPort.postMessage({ type: 'result', value: result });
      } catch (e) {
        parentPort.postMessage({ type: 'error', value: e.message });
      } finally {
        try { isolate?.dispose(); } catch(e) {}
      }
    }
  });

  // Verify our own permissions are restricted
  const fs = await import('node:fs');
  const results = {};

  try { fs.writeFileSync('/tmp/test', 'x'); results.write = 'ALLOWED'; }
  catch (e) { results.write = `BLOCKED (${e.code})`; }

  try { const { execSync } = await import('node:child_process'); execSync('echo hi'); results.spawn = 'ALLOWED'; }
  catch (e) { results.spawn = `BLOCKED (${e.code})`; }

  parentPort.postMessage({ type: 'log', args: ['Worker permissions:', JSON.stringify(results)] });
}
