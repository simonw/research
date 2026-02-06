import test from 'node:test';
import assert from 'node:assert/strict';
import { buildIr } from '../dist/ir-builder.js';

// minimal ParsedApiRequest shape for tests
function req(overrides={}) {
  return {
    method: 'GET',
    url: 'https://example.com/api/items/123?cursor=abc',
    path: '/api/items/123?cursor=abc',
    domain: 'example.com',
    status: 200,
    requestHeaders: { 'x-test': '1', ':authority': 'x' },
    responseContentType: 'application/json',
    responseBody: JSON.stringify([{ id: 1 }, { id: 2 }]),
    timestamp: new Date().toISOString(),
    ...overrides,
  };
}

test('buildIr normalizes path ids and strips pseudo headers', () => {
  const ir = buildIr('https://example.com', [req()], { authMethod: 'Cookie-based', cookies: { a: 'b' }, authHeaders: {} });
  assert.equal(ir.endpoints.length, 1);
  const ep = ir.endpoints[0];
  assert.ok(ep.pathPattern.includes('{id}'));
  assert.ok(!Object.keys(ep.variants[0].requestHeadersSample).some((k) => k.startsWith(':')));
});

test('buildIr deprioritizes html-like GET navigations', () => {
  const ir = buildIr('https://example.com', [req({ responseContentType: 'text/html', responseBody: '<!doctype html><html></html>' })], { authMethod: 'Unknown', cookies: {}, authHeaders: {} });
  assert.ok(ir.endpoints[0].score < 0);
});
