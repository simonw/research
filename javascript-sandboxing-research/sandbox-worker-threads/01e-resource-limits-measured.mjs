// Test: resourceLimits with actual memory measurement
import { Worker, isMainThread, parentPort, workerData } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: resourceLimits with memory measurement ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    resourceLimits: {
      maxOldGenerationSizeMb: 48,
      maxYoungGenerationSizeMb: 16,
    }
  });

  worker.on('message', (msg) => console.log('Worker:', msg));
  worker.on('error', (err) => console.log('Worker ERROR:', err.message));
  worker.on('exit', (code) => console.log('Worker exited with code:', code));

} else {
  const data = [];
  for (let i = 0; ; i++) {
    // Allocate objects on V8 heap
    data.push(Array.from({length: 100}, () => ({ k: Math.random(), v: String(Math.random()) })));
    if (i % 50 === 0) {
      const mem = process.memoryUsage();
      parentPort.postMessage(
        `heapUsed: ${(mem.heapUsed / 1024 / 1024).toFixed(1)}MB, ` +
        `heapTotal: ${(mem.heapTotal / 1024 / 1024).toFixed(1)}MB, ` +
        `rss: ${(mem.rss / 1024 / 1024).toFixed(1)}MB, ` +
        `external: ${(mem.external / 1024 / 1024).toFixed(1)}MB`
      );
    }
  }
}
