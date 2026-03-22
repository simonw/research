// Test: What happens when vm sandbox is escaped but permission model is active?
// Key question: does the permission model protect us even after a vm escape?
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: vm escape + permission model defense in depth ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    execArgv: [
      '--experimental-permission',
      '--allow-fs-read=*',
    ],
  });

  worker.on('message', (msg) => {
    if (msg.type === 'log') console.log('  [sandbox]', ...msg.args);
    else console.log('  [worker]', JSON.stringify(msg));
  });
  worker.on('error', (err) => console.log('Worker ERROR:', err.message));
  worker.on('exit', (code) => console.log('Worker exited:', code));

  setTimeout(() => {
    worker.postMessage({
      type: 'execute',
      code: `
        // Deliberately provide a function that can be used to escape
        const escapedGlobal = log.constructor('return this')();

        // Now we have access to the real global scope
        log('Escaped! typeof globalThis: ' + typeof escapedGlobal);
        log('process available: ' + !!escapedGlobal.process);

        if (escapedGlobal.process) {
          log('process.version: ' + escapedGlobal.process.version);

          // Try to use escaped access for dangerous operations
          try {
            const fs = escapedGlobal.process.mainModule.require('fs');
            fs.writeFileSync('/tmp/evil', 'hacked');
            log('FILE WRITE: SUCCEEDED (bad!)');
          } catch(e) {
            log('File write blocked: ' + e.message);
          }

          try {
            const cp = escapedGlobal.process.mainModule.require('child_process');
            cp.execSync('whoami');
            log('EXEC: SUCCEEDED (bad!)');
          } catch(e) {
            log('Exec blocked: ' + e.message);
          }

          try {
            const fs = escapedGlobal.process.mainModule.require('fs');
            const content = fs.readFileSync('/etc/passwd', 'utf8');
            log('READ /etc/passwd: ' + content.split('\\n')[0]);
          } catch(e) {
            log('Read /etc/passwd: ' + e.message);
          }
        }

        'escape test complete';
      `,
      timeout: 5000,
    });
  }, 500);

  setTimeout(() => worker.postMessage({ type: 'shutdown' }), 5000);

} else {
  const vm = (await import('node:vm')).default;

  parentPort.on('message', (msg) => {
    if (msg.type === 'shutdown') process.exit(0);

    if (msg.type === 'execute') {
      try {
        const sandbox = Object.create(null);
        // Intentionally provide a real function (which can be used to escape vm)
        sandbox.log = (...args) => parentPort.postMessage({ type: 'log', args });
        const context = vm.createContext(sandbox);
        const result = vm.runInContext(msg.code, context, { timeout: msg.timeout || 5000 });
        parentPort.postMessage({ type: 'result', value: result });
      } catch (e) {
        parentPort.postMessage({ type: 'error', message: e.message });
      }
    }
  });
}
