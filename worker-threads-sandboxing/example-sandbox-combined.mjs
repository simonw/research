// example-sandbox-combined.mjs
// Defense-in-depth: Worker + Permission Model + VM + Resource Limits
//
// This example demonstrates the strongest sandboxing achievable with
// built-in Node.js v22 APIs (no third-party packages).

import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Defense-in-Depth Sandboxing Demo (Node.js v22) ===\n');

  // The untrusted code to execute
  const untrustedCode = `
    console.log("Hello from sandboxed code!");
    result = 2 + 2;
    
    // These will all fail or be neutered:
    try { require('fs'); } catch(e) { console.log("require blocked:", e.message); }
    try {
      const fs = this.constructor.constructor("return require('fs')")();
      // Got the module, but permission model blocks actual operations:
      try {
        fs.readFileSync('/etc/passwd');
        console.log("DANGER: read /etc/passwd!");
      } catch(e2) {
        console.log("fs module obtained but read blocked:", e2.code);
      }
    } catch(e) { console.log("fs via prototype escape blocked:", e.message); }
  `;

  const worker = new Worker(`
    const { parentPort, workerData } = require('node:worker_threads');
    const vm = require('node:vm');

    // Create a minimal sandbox context
    const sandbox = {
      console: {
        log: (...args) => parentPort.postMessage({ type: 'log', args }),
        error: (...args) => parentPort.postMessage({ type: 'error', args }),
      },
      result: undefined,
      // Do NOT expose: require, process, global, Buffer, etc.
    };
    vm.createContext(sandbox);

    try {
      vm.runInContext(workerData.code, sandbox, {
        timeout: 5000,           // 5 second timeout
        filename: 'sandbox.js',  // For stack traces
      });
      parentPort.postMessage({ type: 'result', value: sandbox.result });
    } catch (err) {
      parentPort.postMessage({ type: 'error', args: [err.message] });
    }
  `, {
    eval: true,
    workerData: { code: untrustedCode },
    
    // Layer 1: Environment isolation - strip all env vars
    env: {},
    
    // Layer 2: Permission model - restrict OS-level access
    execArgv: [
      '--permission',
      // Grant no fs, no child_process, no worker, no net
    ],
    
    // Layer 3: Resource limits - prevent memory exhaustion
    resourceLimits: {
      maxOldGenerationSizeMb: 64,
      maxYoungGenerationSizeMb: 16,
      stackSizeMb: 4,
    },
  });

  worker.on('message', (msg) => {
    if (msg.type === 'log') console.log('  [sandbox]', ...msg.args);
    else if (msg.type === 'error') console.error('  [sandbox error]', ...msg.args);
    else if (msg.type === 'result') console.log('  [result]', msg.value);
  });

  worker.on('error', (err) => {
    console.error('  Worker crashed:', err.message);
  });

  worker.on('exit', (code) => {
    console.log(`\n  Worker exited with code ${code}`);
    if (code !== 0) console.log('  (Non-zero exit indicates abnormal termination)');
  });
}
