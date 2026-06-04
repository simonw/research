// Test: CPU monopolization prevention
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: CPU monopolization prevention ===\n');

  // Test 1: Worker with infinite loop - can we kill it?
  console.log('Test 1: Killing a worker stuck in infinite loop...');
  const worker1 = new Worker(`
    const { parentPort } = require('node:worker_threads');
    parentPort.postMessage('starting infinite loop');
    while(true) {} // infinite loop
  `, { eval: true });

  worker1.on('message', (msg) => console.log('  Worker1:', msg));
  worker1.on('exit', (code) => console.log('  Worker1 exited with code:', code));
  worker1.on('error', (err) => console.log('  Worker1 error:', err.message));

  // Kill after 1 second
  setTimeout(async () => {
    console.log('  Terminating worker1...');
    const ret = await worker1.terminate();
    console.log('  worker1.terminate() returned:', ret);
  }, 1000);

  // Test 2: vm.runInContext with timeout
  setTimeout(() => {
    console.log('\nTest 2: vm timeout for CPU-bound code...');
    const worker2 = new Worker(`
      const { parentPort } = require('node:worker_threads');
      const vm = require('node:vm');

      const context = vm.createContext({});

      try {
        vm.runInContext('while(true) {}', context, { timeout: 1000 });
        parentPort.postMessage('should not reach here');
      } catch(e) {
        parentPort.postMessage('Timeout caught: ' + e.message);
      }
    `, { eval: true });

    worker2.on('message', (msg) => console.log('  Worker2:', msg));
    worker2.on('exit', (code) => console.log('  Worker2 exited with code:', code));
  }, 3000);

  // Test 3: AbortController with worker (Node 18+)
  setTimeout(() => {
    console.log('\nTest 3: Using AbortController to signal worker...');
    const ac = new AbortController();

    // Note: there's no built-in AbortSignal support for Worker termination
    // We have to implement it ourselves
    const worker3 = new Worker(`
      const { parentPort } = require('node:worker_threads');
      let count = 0;
      const interval = setInterval(() => {
        count++;
        parentPort.postMessage('tick ' + count);
      }, 200);

      parentPort.on('message', (msg) => {
        if (msg === 'stop') {
          clearInterval(interval);
          parentPort.postMessage('stopped gracefully');
          process.exit(0);
        }
      });
    `, { eval: true });

    worker3.on('message', (msg) => console.log('  Worker3:', msg));
    worker3.on('exit', (code) => console.log('  Worker3 exited with code:', code));

    setTimeout(() => {
      console.log('  Sending stop signal to worker3...');
      worker3.postMessage('stop');
    }, 1000);
  }, 6000);
}
