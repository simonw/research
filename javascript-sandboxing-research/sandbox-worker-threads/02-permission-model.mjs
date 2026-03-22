// Test: Can we use Node.js Permission Model via execArgv on a Worker?
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Test: Permission Model via execArgv ===\n');

  // Attempt 1: Pass --experimental-permission to worker
  try {
    const worker = new Worker(new URL(import.meta.url), {
      execArgv: ['--experimental-permission', '--allow-fs-read=*'],
    });
    worker.on('message', (msg) => console.log('Worker message:', msg));
    worker.on('error', (err) => console.log('Worker error:', err.message));
    worker.on('exit', (code) => console.log('Worker exited with code:', code));
  } catch (e) {
    console.log('Failed to create worker with permission flags:', e.message);
  }

} else {
  // Try various operations
  const fs = await import('node:fs');
  const results = {};

  // Test filesystem read
  try {
    fs.readFileSync('/etc/hostname', 'utf8');
    results.fsRead = 'ALLOWED';
  } catch (e) {
    results.fsRead = `BLOCKED: ${e.code || e.message}`;
  }

  // Test filesystem write
  try {
    fs.writeFileSync('/tmp/test-sandbox-write', 'test');
    results.fsWrite = 'ALLOWED';
    fs.unlinkSync('/tmp/test-sandbox-write');
  } catch (e) {
    results.fsWrite = `BLOCKED: ${e.code || e.message}`;
  }

  // Test network
  try {
    const resp = await fetch('https://httpbin.org/get');
    results.network = `ALLOWED (status: ${resp.status})`;
  } catch (e) {
    results.network = `BLOCKED: ${e.code || e.message}`;
  }

  // Test child_process
  try {
    const { execSync } = await import('node:child_process');
    execSync('echo hello');
    results.childProcess = 'ALLOWED';
  } catch (e) {
    results.childProcess = `BLOCKED: ${e.code || e.message}`;
  }

  parentPort.postMessage(results);
}
