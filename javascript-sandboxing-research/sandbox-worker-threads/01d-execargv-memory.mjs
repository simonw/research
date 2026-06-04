// Test: Memory limit via --max-old-space-size in execArgv
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: --max-old-space-size via execArgv ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    execArgv: ['--max-old-space-size=32'],
    resourceLimits: {
      maxOldGenerationSizeMb: 32,
      maxYoungGenerationSizeMb: 8,
    }
  });

  worker.on('message', (msg) => console.log('Worker:', msg));
  worker.on('error', (err) => console.log('Worker ERROR:', err.message));
  worker.on('exit', (code) => console.log('Worker exited with code:', code));

} else {
  parentPort.postMessage('Allocating unique heap data with --max-old-space-size=32...');
  const data = [];
  let count = 0;
  try {
    for (let i = 0; ; i++) {
      const arr = new Array(1000);
      for (let j = 0; j < 1000; j++) {
        arr[j] = Math.random().toString(36);
      }
      data.push(arr);
      count = i;
      if (i % 100 === 0) {
        parentPort.postMessage(`Batch ${i} (${i * 1000} strings)`);
      }
    }
  } catch (e) {
    parentPort.postMessage(`OOM at batch ${count}: ${e.message}`);
  }
}
