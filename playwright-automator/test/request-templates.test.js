import test from 'node:test';
import assert from 'node:assert/strict';
import { buildRequestTemplates } from '../dist/request-templates.js';

const req = {
  method: 'GET',
  url: 'https://example.com/api/items',
  path: '/api/items',
  domain: 'example.com',
  status: 200,
  requestHeaders: {
    'authorization': 'Bearer secret',
    'cookie': 'a=b',
    'x-test': '1',
    ':authority': 'x',
  },
  responseContentType: 'application/json',
  responseBody: '[]',
};

test('buildRequestTemplates strips sensitive headers and pseudo headers', () => {
  const t = buildRequestTemplates([req], 5);
  assert.equal(t.length, 1);
  assert.ok(!('authorization' in t[0].headers));
  assert.ok(!('cookie' in t[0].headers));
  assert.ok(!Object.keys(t[0].headers).some((k) => k.startsWith(':')));
  assert.equal(t[0].headers['x-test'], '1');
});
