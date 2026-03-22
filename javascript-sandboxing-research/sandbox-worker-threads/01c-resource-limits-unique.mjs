// Test: resourceLimits with truly unique V8 heap allocations
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: resourceLimits with unique heap data ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    resourceLimits: {
      maxOldGenerationSizeMb: 32,
      maxYoungGenerationSizeMb: 8,
    }
  });

  worker.on('message', (msg) => console.log('Worker:', msg));
  worker.on('error', (err) => console.log('Worker ERROR:', err.message));
  worker.on('exit', (code) => console.log('Worker exited with code:', code));

} else {
  parentPort.postMessage('Allocating unique heap data...');
  const data = [];
  let totalMB = 0;
  try {
    for (let i = 0; ; i++) {
      // Create unique strings that can't be deduplicated
      const arr = new Array(1000);
      for (let j = 0; j < 1000; j++) {
        arr[j] = Math.random().toString(36);
      }
      data.push(arr);
      totalMB = (i * 1000 * 10) / (1024 * 1024); // ~10 bytes per random string
      if (i % 100 === 0) {
        parentPort.postMessage(`~${totalMB.toFixed(1)} MB allocated (${i * 1000} strings)`);
      }
    }
  } catch (e) {
    parentPort.postMessage(`Caught: ${e.message} after ~${totalMB.toFixed(1)} MB`);
  }
}
