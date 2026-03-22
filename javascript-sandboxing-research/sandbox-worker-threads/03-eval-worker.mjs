// Test: Running untrusted code via eval option + vm module inside worker
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: eval mode Worker + vm sandboxing ===\n');

  // Worker code as a string - simulating untrusted code execution
  const workerCode = `
    const { parentPort } = require('node:worker_threads');
    const vm = require('node:vm');

    // Create a restricted context
    const context = vm.createContext({
      console: { log: (...args) => parentPort.postMessage({ type: 'log', args }) },
      setTimeout: setTimeout,
      // Intentionally NOT providing: require, process, fs, etc.
    });

    // Run "untrusted" code in the restricted context
    const untrustedCode = \`
      console.log('Hello from sandboxed code!');

      // Try to escape the sandbox
      let escaped = false;

      // Attempt 1: Access process
      try { process.exit(1); } catch(e) { console.log('process.exit blocked: ' + e.message); }

      // Attempt 2: Access require
      try { require('fs'); } catch(e) { console.log('require blocked: ' + e.message); }

      // Attempt 3: Access global this
      try {
        const g = this.constructor.constructor('return this')();
        if (g.process) {
          console.log('ESCAPED via constructor trick! process available');
          escaped = true;
        } else {
          console.log('constructor trick: got global but no process');
        }
      } catch(e) {
        console.log('constructor trick blocked: ' + e.message);
      }

      // Attempt 4: Access via Function constructor
      try {
        const fn = new Function('return process');
        const p = fn();
        console.log('ESCAPED via Function constructor! process: ' + typeof p);
        escaped = true;
      } catch(e) {
        console.log('Function constructor blocked: ' + e.message);
      }

      console.log('Escape attempts completed. Escaped: ' + escaped);
    \`;

    try {
      vm.runInContext(untrustedCode, context, { timeout: 2000 });
    } catch (e) {
      parentPort.postMessage({ type: 'error', message: e.message });
    }

    parentPort.postMessage({ type: 'done' });
  `;

  const worker = new Worker(workerCode, { eval: true });
  worker.on('message', (msg) => {
    if (msg.type === 'log') console.log('  [sandbox]', ...msg.args);
    else console.log('  [worker]', msg.type, msg.message || '');
  });
  worker.on('error', (err) => console.log('Worker error:', err.message));
  worker.on('exit', (code) => console.log('Worker exited with code:', code));

} else {
  // This branch won't execute - we're using eval mode
}
