// Test: Permission model + --allow-addons to load isolated-vm
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: Permission Model + --allow-addons + isolated-vm ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    execArgv: [
      '--experimental-permission',
      '--allow-fs-read=*',
      '--allow-addons',
      // Still no: --allow-fs-write, --allow-child-process, --allow-worker
    ],
  });

  worker.on('message', (msg) => console.log('Worker:', msg));
  worker.on('error', (err) => console.log('Worker ERROR:', err.message));
  worker.on('exit', (code) => console.log('Worker exited:', code));
} else {
  const results = {};

  // Load isolated-vm
  try {
    const ivm = (await import('isolated-vm')).default;
    results.ivmLoad = 'OK';

    const isolate = new ivm.Isolate({ memoryLimit: 16 });
    const context = await isolate.createContext();
    const script = await isolate.compileScript('2 + 2');
    const result = await script.run(context, { timeout: 1000 });
    results.ivmRun = 'OK, result=' + result;
    isolate.dispose();
  } catch (e) {
    results.ivmLoad = 'FAILED: ' + e.message;
  }

  // Test restrictions
  const fs = await import('node:fs');
  try { fs.writeFileSync('/tmp/test', 'x'); results.fsWrite = 'ALLOWED'; }
  catch (e) { results.fsWrite = 'BLOCKED (' + e.code + ')'; }

  try { const { execSync } = await import('node:child_process'); execSync('echo hi'); results.exec = 'ALLOWED'; }
  catch (e) { results.exec = 'BLOCKED (' + e.code + ')'; }

  parentPort.postMessage(JSON.stringify(results, null, 2));
}
