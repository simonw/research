// Test: Permission model + vm module (no native addons needed)
// This is the "pure Node.js" approach
import { Worker, isMainThread, parentPort } from 'node:worker_threads';
import vm from 'node:vm';

if (isMainThread) {
  console.log('=== Test: Permission Model + vm module (pure Node.js) ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    execArgv: [
      '--experimental-permission',
      '--allow-fs-read=*',
      // No write, no child_process, no worker, no addons
    ],
  });

  worker.on('message', (msg) => {
    if (msg.type === 'log') console.log('  [sandbox]', ...msg.args);
    else console.log('  [worker]', JSON.stringify(msg));
  });
  worker.on('error', (err) => console.log('Worker ERROR:', err.message));
  worker.on('exit', (code) => console.log('Worker exited:', code));

  // Send code to execute
  setTimeout(() => {
    worker.postMessage({
      type: 'execute',
      code: `
        log('Hello from vm sandbox!');

        // Try escape via constructor
        try {
          const g = this.constructor.constructor('return this')();
          if (g.process) log('ESCAPED via constructor!');
          else log('constructor trick: got global but no process');
        } catch(e) { log('constructor blocked: ' + e.message); }

        // Even if we escape vm, the permission model blocks dangerous ops
        42;
      `,
      timeout: 2000,
    });
  }, 500);

  // Test CPU timeout via vm
  setTimeout(() => {
    worker.postMessage({
      type: 'execute',
      code: 'while(true) {}',
      timeout: 1000,
    });
  }, 3000);

  setTimeout(() => worker.postMessage({ type: 'shutdown' }), 6000);

} else {
  const vm = (await import('node:vm')).default;

  // Verify permissions
  const fs = await import('node:fs');
  const results = {};

  try { fs.writeFileSync('/tmp/test-perm', 'x'); results.fsWrite = 'ALLOWED'; }
  catch (e) { results.fsWrite = 'BLOCKED (' + e.code + ')'; }

  try { const { execSync } = await import('node:child_process'); execSync('echo hi'); results.exec = 'ALLOWED'; }
  catch (e) { results.exec = 'BLOCKED (' + e.code + ')'; }

  parentPort.postMessage({ type: 'permissions', ...results });

  parentPort.on('message', (msg) => {
    if (msg.type === 'shutdown') process.exit(0);

    if (msg.type === 'execute') {
      try {
        // Create a context with minimal API surface
        const sandbox = Object.create(null);
        sandbox.log = (...args) => parentPort.postMessage({ type: 'log', args });
        // Note: Even if code escapes vm, the permission model still protects fs/net/exec
        const context = vm.createContext(sandbox);
        const result = vm.runInContext(msg.code, context, { timeout: msg.timeout || 5000 });
        parentPort.postMessage({ type: 'result', value: result });
      } catch (e) {
        parentPort.postMessage({ type: 'error', message: e.message });
      }
    }
  });
}
