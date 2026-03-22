// example-env-isolation.mjs
// Demonstrates environment variable isolation options

import { Worker, isMainThread, parentPort, SHARE_ENV } from 'node:worker_threads';

if (isMainThread) {
  process.env.MAIN_SECRET = 'top-secret-value';

  console.log('=== Environment Isolation Demo ===\n');

  // Worker with custom env (sandboxed - no access to parent env)
  console.log('--- Worker with custom env (sandboxed) ---');
  const w1 = new Worker(new URL(import.meta.url), {
    env: { ALLOWED_VAR: 'hello' },  // Only this var is available
  });
  w1.on('message', (msg) => console.log('Sandboxed worker:', msg));
  await new Promise(r => w1.on('exit', r));

  // Worker with default env (copy of parent env)
  console.log('\n--- Worker with default env (copy) ---');
  const w2 = new Worker(new URL(import.meta.url), {
    // env defaults to process.env (a copy)
  });
  w2.on('message', (msg) => console.log('Default worker:', msg));
  await new Promise(r => w2.on('exit', r));

  // Worker with SHARE_ENV (shared - changes visible both ways)
  console.log('\n--- Worker with SHARE_ENV (shared) ---');
  const w3 = new Worker(new URL(import.meta.url), {
    env: SHARE_ENV,
  });
  w3.on('message', (msg) => console.log('Shared worker:', msg));
  await new Promise(r => w3.on('exit', r));
  console.log('After shared worker, WORKER_SET =', process.env.WORKER_SET);

} else {
  const report = {
    MAIN_SECRET: process.env.MAIN_SECRET || '(not available)',
    ALLOWED_VAR: process.env.ALLOWED_VAR || '(not available)',
    PATH: process.env.PATH ? '(available)' : '(not available)',
  };
  process.env.WORKER_SET = 'from-worker';
  parentPort.postMessage(JSON.stringify(report));
}
