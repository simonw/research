// Test: Worker resourceLimits - what happens when memory is exceeded?
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: Worker resourceLimits ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    resourceLimits: {
      maxOldGenerationSizeMb: 16,   // V8 old generation heap
      maxYoungGenerationSizeMb: 4,  // V8 young generation heap
      codeRangeSizeMb: 4,           // V8 code range
      stackSizeMb: 2,               // Stack size
    }
  });

  worker.on('message', (msg) => console.log('Worker message:', msg));
  worker.on('error', (err) => console.log('Worker error:', err.message));
  worker.on('exit', (code) => console.log('Worker exited with code:', code));

  // Also test: what if we DON'T set limits?
  setTimeout(() => {
    console.log('\n=== Test: Worker WITHOUT resourceLimits ===\n');
    const worker2 = new Worker(new URL(import.meta.url), {
      workerData: { noAlloc: true }
    });
    worker2.on('message', (msg) => console.log('Worker2 message:', msg));
    worker2.on('exit', (code) => console.log('Worker2 exited with code:', code));
  }, 3000);

} else {
  const { workerData } = await import('node:worker_threads').then(m => ({ workerData: m.workerData }));

  if (workerData?.noAlloc) {
    parentPort.postMessage('Worker2 started without allocating');
    process.exit(0);
  }

  // Try to allocate more memory than allowed
  parentPort.postMessage('Starting memory allocation...');
  const arrays = [];
  try {
    for (let i = 0; i < 1000; i++) {
      arrays.push(new ArrayBuffer(1024 * 1024)); // 1MB each
      if (i % 5 === 0) {
        parentPort.postMessage(`Allocated ${i + 1} MB`);
      }
    }
  } catch (e) {
    parentPort.postMessage(`Caught error after allocating: ${e.message}`);
  }
}
