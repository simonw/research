// Debug: what paths does isolated-vm need?
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  // First try: allow everything to see if it works at all
  const worker = new Worker(new URL(import.meta.url), {
    execArgv: [
      '--experimental-permission',
      '--allow-fs-read=*',
    ],
  });

  worker.on('message', (msg) => console.log('Worker:', msg));
  worker.on('error', (err) => console.log('Worker ERROR:', err.message));
  worker.on('exit', (code) => console.log('Worker exited:', code));
} else {
  try {
    const ivm = (await import('isolated-vm')).default;
    parentPort.postMessage('isolated-vm loaded successfully');

    const isolate = new ivm.Isolate({ memoryLimit: 16 });
    const context = await isolate.createContext();
    const jail = context.global;
    await jail.set('log', new ivm.Callback((...args) => parentPort.postMessage('LOG: ' + args.join(' '))));

    const script = await isolate.compileScript('log("hello from isolate"); 42;');
    const result = await script.run(context, { timeout: 1000 });
    parentPort.postMessage('Result: ' + result);

    // Check permissions
    const fs = await import('node:fs');
    try { fs.writeFileSync('/tmp/test', 'x'); parentPort.postMessage('write: ALLOWED'); }
    catch (e) { parentPort.postMessage('write: BLOCKED - ' + e.code); }

    try { const { execSync } = await import('node:child_process'); execSync('echo hi'); parentPort.postMessage('exec: ALLOWED'); }
    catch (e) { parentPort.postMessage('exec: BLOCKED - ' + e.code); }

    isolate.dispose();
  } catch (e) {
    parentPort.postMessage('ERROR: ' + e.message);
  }
}
