// Control test: Worker WITHOUT permission model flags
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
  console.log('=== Control: Worker WITHOUT permission flags ===\n');

  const worker = new Worker(new URL(import.meta.url), {
    // No execArgv with permission flags
  });
  worker.on('message', (msg) => console.log('Worker:', JSON.stringify(msg, null, 2)));
  worker.on('error', (err) => console.log('Worker error:', err.message));
  worker.on('exit', (code) => console.log('Worker exited:', code));
} else {
  const fs = await import('node:fs');
  const results = {};

  try { fs.readFileSync('/etc/hostname', 'utf8'); results.fsRead = 'ALLOWED'; }
  catch (e) { results.fsRead = `BLOCKED: ${e.code}`; }

  try { fs.writeFileSync('/tmp/test-sandbox-write', 'test'); results.fsWrite = 'ALLOWED'; fs.unlinkSync('/tmp/test-sandbox-write'); }
  catch (e) { results.fsWrite = `BLOCKED: ${e.code}`; }

  try { await fetch('https://example.com'); results.network = 'ALLOWED'; }
  catch (e) { results.network = `BLOCKED: ${e.code || e.message}`; }

  try { const { execSync } = await import('node:child_process'); execSync('echo hello'); results.childProcess = 'ALLOWED'; }
  catch (e) { results.childProcess = `BLOCKED: ${e.code}`; }

  parentPort.postMessage(results);
}
