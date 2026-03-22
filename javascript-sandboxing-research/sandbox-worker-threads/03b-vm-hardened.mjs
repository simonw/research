// Test: Can we harden vm.createContext against the constructor escape?
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: Hardened vm context ===\n');

  const workerCode = `
    const { parentPort } = require('node:worker_threads');
    const vm = require('node:vm');

    // Approach 1: Use vm.createContext with Object.create(null) as the sandbox
    // This removes the prototype chain
    const sandbox = Object.create(null);
    sandbox.console = { log: (...args) => parentPort.postMessage({ type: 'log', args }) };

    const context = vm.createContext(sandbox);

    const untrustedCode = \`
      // Attempt constructor trick
      try {
        const g = this.constructor.constructor('return this')();
        if (g.process) {
          console.log('ESCAPED via constructor trick!');
        } else {
          console.log('constructor trick: got something but no process');
        }
      } catch(e) {
        console.log('constructor trick BLOCKED: ' + e.message);
      }

      // Attempt via console.log.constructor
      try {
        const g = console.log.constructor('return this')();
        if (g.process) {
          console.log('ESCAPED via console.log.constructor!');
        } else {
          console.log('console.log.constructor: got something but no process');
        }
      } catch(e) {
        console.log('console.log.constructor BLOCKED: ' + e.message);
      }

      // Attempt via arguments.callee
      try {
        (function() {
          const g = arguments.callee.caller.constructor('return this')();
          console.log('ESCAPED via arguments.callee! process: ' + !!g.process);
        })();
      } catch(e) {
        console.log('arguments.callee BLOCKED: ' + e.message);
      }

      console.log('All escape attempts completed');
    \`;

    try {
      vm.runInContext(untrustedCode, context, { timeout: 2000 });
    } catch(e) {
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
}
