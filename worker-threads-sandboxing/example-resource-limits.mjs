// example-resource-limits.mjs
// Demonstrates Worker resourceLimits and what happens when they are exceeded

import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Resource Limits Demo ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    resourceLimits: {
      maxOldGenerationSizeMb: 16,    // 16MB heap limit
      maxYoungGenerationSizeMb: 4,   // 4MB young generation
      stackSizeMb: 2,                // 2MB stack
    },
  });

  worker.on('error', (err) => {
    console.log('Worker error event:', err.message);
    console.log('Error code:', err.code);
  });

  worker.on('exit', (code) => {
    console.log('Worker exited with code:', code);
    console.log('(code 1 indicates abnormal termination due to resource limit)\n');
  });

  worker.on('message', (msg) => {
    console.log('Worker message:', msg);
  });
} else {
  // Inside the worker: try to exhaust memory
  parentPort.postMessage('Starting memory allocation...');
  const arrays = [];
  try {
    while (true) {
      arrays.push(new Array(1024 * 1024).fill('x'));
    }
  } catch (e) {
    // This may or may not be reached - the worker might be killed first
    parentPort.postMessage(`Caught error: ${e.message}`);
  }
}
