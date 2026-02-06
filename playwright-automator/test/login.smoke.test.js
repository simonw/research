import test from 'node:test';
import assert from 'node:assert/strict';
import { getAuthProfilePaths } from '../dist/auth-profiles.js';

test('auth profile paths are stable', () => {
  const p = getAuthProfilePaths('example.com', 'default', process.cwd());
  assert.ok(p.storageStatePath.endsWith('/auth-profiles/example.com/default/storageState.json'));
});
