// Test: Very small resourceLimits (4MB) to see if it ever kicks in
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: Tiny resourceLimits (4MB old gen) ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    resourceLimits: {
      maxOldGenerationSizeMb: 4,
      maxYoungGenerationSizeMb: 2,
    }
  });

  worker.on('message', (msg) => console.log('Worker:', msg));
  worker.on('error', (err) => console.log('Worker ERROR:', err.constructor.name, '-', err.message));
  worker.on('exit', (code) => console.log('Worker exited with code:', code));

} else {
  const data = [];
  try {
    for (let i = 0; ; i++) {
      data.push(Array.from({length: 100}, () => ({ k: Math.random(), v: String(Math.random()) })));
      if (i % 10 === 0) {
        const mem = process.memoryUsage();
        parentPort.postMessage(
          `heap: ${(mem.heapUsed / 1024 / 1024).toFixed(1)}MB / rss: ${(mem.rss / 1024 / 1024).toFixed(1)}MB`
        );
      }
    }
  } catch (e) {
    parentPort.postMessage(`ERROR: ${e.message}`);
  }
}
