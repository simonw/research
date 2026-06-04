// Test: resourceLimits with ACTUAL V8 heap allocations (not ArrayBuffer)
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: resourceLimits with V8 heap objects ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    resourceLimits: {
      maxOldGenerationSizeMb: 16,
      maxYoungGenerationSizeMb: 4,
    }
  });

  worker.on('message', (msg) => console.log('Worker:', msg));
  worker.on('error', (err) => console.log('Worker ERROR:', err.message));
  worker.on('exit', (code) => console.log('Worker exited with code:', code));

} else {
  // Use V8 heap objects (arrays of objects) instead of ArrayBuffer
  parentPort.postMessage('Allocating V8 heap objects...');
  const data = [];
  try {
    for (let i = 0; ; i++) {
      // Each object lives on the V8 heap
      data.push({ index: i, value: 'x'.repeat(1000) });
      if (i % 5000 === 0) {
        parentPort.postMessage(`Created ${i} objects`);
      }
    }
  } catch (e) {
    parentPort.postMessage(`Caught: ${e.message}`);
  }
}
