// Test: Running the MAIN PROCESS with --experimental-permission
// and seeing if workers inherit the restrictions
//
// Run with: node --experimental-permission --allow-fs-read=* --allow-fs-write=/tmp/* 05-permission-main-process.mjs

import { Worker, isMainThread, parentPort } from 'node:worker_threads';
import { permission } from 'node:process';

if (isMainThread) {
  console.log('=== Test: Permission Model on main process ===\n');

  // Check if permission model is active
  if (permission?.has) {
    console.log('Permission model is active!');
    console.log('  fs.read:', permission.has('fs.read'));
    console.log('  fs.write:', permission.has('fs.write'));
    console.log('  fs.write /tmp:', permission.has('fs.write', '/tmp'));
    console.log('  child_process:', permission.has('child_process'));
    console.log('  worker:', permission.has('worker'));
  } else {
    console.log('Permission model is NOT active.');
    console.log('Run with: node --experimental-permission --allow-fs-read=* --allow-worker 05-permission-main-process.mjs');
    process.exit(0);
  }

  // Spawn a worker and see what it can do
  const worker = new Worker(new URL(import.meta.url));
  worker.on('message', (msg) => console.log('Worker:', JSON.stringify(msg, null, 2)));
  worker.on('error', (err) => console.log('Worker error:', err.message));
  worker.on('exit', (code) => console.log('Worker exited:', code));

} else {
  const fs = await import('node:fs');
  const results = {};

  // Test filesystem read
  try {
    fs.readFileSync('/etc/hostname', 'utf8');
    results.fsReadEtc = 'ALLOWED';
  } catch (e) {
    results.fsReadEtc = `BLOCKED: ${e.code}`;
  }

  // Test filesystem write to /tmp
  try {
    fs.writeFileSync('/tmp/sandbox-test', 'hello');
    results.fsWriteTmp = 'ALLOWED';
    fs.unlinkSync('/tmp/sandbox-test');
  } catch (e) {
    results.fsWriteTmp = `BLOCKED: ${e.code}`;
  }

  // Test filesystem write elsewhere
  try {
    fs.writeFileSync('/home/user/sandbox-test', 'hello');
    results.fsWriteHome = 'ALLOWED';
    fs.unlinkSync('/home/user/sandbox-test');
  } catch (e) {
    results.fsWriteHome = `BLOCKED: ${e.code}`;
  }

  // Test network
  try {
    await fetch('https://example.com');
    results.network = 'ALLOWED';
  } catch (e) {
    results.network = `BLOCKED: ${e.code}`;
  }

  // Test child_process
  try {
    const { execSync } = await import('node:child_process');
    execSync('echo hello');
    results.childProcess = 'ALLOWED';
  } catch (e) {
    results.childProcess = `BLOCKED: ${e.code}`;
  }

  parentPort.postMessage(results);
}
