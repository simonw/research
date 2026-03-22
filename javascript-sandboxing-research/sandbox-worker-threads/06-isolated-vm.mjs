// Test: isolated-vm for true V8 isolate sandboxing
import ivm from 'isolated-vm';

console.log('=== Test: isolated-vm ===\n');

// Test 1: Basic isolation
console.log('--- Test 1: Basic isolation ---');
{
  const isolate = new ivm.Isolate({ memoryLimit: 32 }); // 32MB hard limit
  const context = await isolate.createContext();

  // Inject a log function
  const jail = context.global;
  await jail.set('log', new ivm.Callback((...args) => console.log('  [isolate]', ...args)));

  const code = `
    log('Hello from isolate!');

    // Try to access Node.js globals
    try { log('process: ' + typeof process); } catch(e) { log('process access: ' + e.message); }
    try { log('require: ' + typeof require); } catch(e) { log('require access: ' + e.message); }
    try { log('globalThis.process: ' + typeof globalThis.process); } catch(e) { log('globalThis.process: ' + e.message); }
    try { log('import: cannot test dynamic import in isolate'); } catch(e) { log('import: ' + e.message); }

    // Try constructor escape
    try {
      const g = this.constructor.constructor('return this')();
      log('constructor trick: ' + typeof g + ', process: ' + typeof g.process);
    } catch(e) {
      log('constructor trick blocked: ' + e.message);
    }

    log('Done with escape attempts');
  `;

  const script = await isolate.compileScript(code);
  await script.run(context);
  isolate.dispose();
}

// Test 2: Memory limit enforcement
console.log('\n--- Test 2: Memory limit enforcement ---');
{
  const isolate = new ivm.Isolate({ memoryLimit: 16 }); // 16MB
  const context = await isolate.createContext();

  const jail = context.global;
  await jail.set('log', new ivm.Callback((...args) => console.log('  [isolate]', ...args)));

  const code = `
    const data = [];
    for (let i = 0; ; i++) {
      data.push(Array.from({length: 100}, () => ({ k: Math.random(), v: String(Math.random()) })));
      if (i % 50 === 0) {
        log('Batch ' + i);
      }
    }
  `;

  try {
    const script = await isolate.compileScript(code);
    await script.run(context, { timeout: 10000 });
    console.log('  Completed without error (unexpected)');
  } catch (e) {
    console.log('  Caught:', e.constructor.name, '-', e.message);
  }

  try {
    console.log('  Heap stats:', JSON.stringify(isolate.getHeapStatisticsSync()));
  } catch(e) {
    console.log('  Heap stats unavailable (isolate disposed?):', e.message);
  }

  try { isolate.dispose(); } catch(e) {}
}

// Test 3: CPU timeout
console.log('\n--- Test 3: CPU timeout ---');
{
  const isolate = new ivm.Isolate({ memoryLimit: 32 });
  const context = await isolate.createContext();

  const code = `while(true) {}`;

  try {
    const script = await isolate.compileScript(code);
    await script.run(context, { timeout: 1000 }); // 1 second timeout
    console.log('  Completed without error (unexpected)');
  } catch (e) {
    console.log('  Caught:', e.constructor.name, '-', e.message);
  }
  isolate.dispose();
}

// Test 4: No filesystem or network access
console.log('\n--- Test 4: No filesystem/network access ---');
{
  const isolate = new ivm.Isolate({ memoryLimit: 32 });
  const context = await isolate.createContext();
  const jail = context.global;
  await jail.set('log', new ivm.Callback((...args) => console.log('  [isolate]', ...args)));

  const code = `
    try { const fs = require('fs'); log('fs: AVAILABLE'); } catch(e) { log('require: ' + e.message); }
    try { fetch('https://example.com'); } catch(e) { log('fetch: ' + e.message); }
    try { const x = new XMLHttpRequest(); } catch(e) { log('XMLHttpRequest: ' + e.message); }
    log('No I/O primitives available in isolate');
  `;

  const script = await isolate.compileScript(code);
  await script.run(context, { timeout: 2000 });
  isolate.dispose();
}

// Test 5: Using isolated-vm INSIDE a worker thread
console.log('\n--- Test 5: isolated-vm inside Worker thread ---');
{
  const { Worker } = await import('node:worker_threads');

  const workerCode = `
    const { parentPort } = require('node:worker_threads');
    const ivm = require('isolated-vm');

    const isolate = new ivm.Isolate({ memoryLimit: 16 });
    const context = isolate.createContextSync();
    const jail = context.global;
    jail.setSync('log', new ivm.Callback((...args) => parentPort.postMessage({ type: 'log', args })));

    const script = isolate.compileScriptSync('log("Hello from isolate inside worker!"); 2 + 2;');
    const result = script.runSync(context, { timeout: 1000 });
    parentPort.postMessage({ type: 'result', value: result });

    isolate.dispose();
  `;

  const worker = new Worker(workerCode, { eval: true });
  worker.on('message', (msg) => {
    if (msg.type === 'log') console.log('  [worker/isolate]', ...msg.args);
    else console.log('  [worker]', msg.type + ':', msg.value);
  });
  worker.on('exit', (code) => console.log('  Worker exited:', code));

  await new Promise(resolve => worker.on('exit', resolve));
}

console.log('\n=== All isolated-vm tests complete ===');
