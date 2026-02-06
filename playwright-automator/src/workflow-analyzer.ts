/**
 * Workflow Analyzer — Detect list→detail patterns, pagination, and variable flow.
 *
 * Scans the recorded API requests to find:
 * 1. List→Detail pairs: A base endpoint + a parameterized sub-path
 * 2. Pagination: Repeated calls to the same endpoint with changing offset/cursor params
 * 3. Variable flow: IDs from response bodies appearing in subsequent request URLs
 */

import type { ParsedApiRequest, WorkflowAnalysis, WorkflowPattern } from './types.js';
import { normalizePath } from './ir-builder.js';

/** Fields that commonly hold IDs. */
const ID_FIELD_NAMES = new Set([
  'id', '_id', 'uuid', 'slug', 'key', 'pk',
  'itemId', 'item_id', 'postId', 'post_id',
  'userId', 'user_id', 'threadId', 'thread_id',
  'conversationId', 'conversation_id', 'messageId', 'message_id',
  'projectId', 'project_id', 'documentId', 'document_id',
]);

/** Common pagination parameter names. */
const PAGINATION_PARAMS = new Set([
  'page', 'offset', 'cursor', 'after', 'before',
  'skip', 'start', 'from', 'next', 'nextToken',
  'continuation', 'pageToken', 'page_token',
  'startCursor', 'start_cursor', 'endCursor', 'end_cursor',
]);

/**
 * Analyze recorded API requests for workflow patterns.
 */
export function analyzeWorkflow(apiRequests: ParsedApiRequest[]): WorkflowAnalysis {
  const patterns: WorkflowPattern[] = [];

  patterns.push(...detectListDetailPairs(apiRequests));
  patterns.push(...detectPagination(apiRequests));
  patterns.push(...detectVariableFlow(apiRequests));

  const summary = renderWorkflowSummary(patterns);
  return { patterns, summary };
}

/**
 * Detect list→detail endpoint pairs.
 * Looks for a base path (e.g. /api/items) that is a prefix of a parameterized
 * path (e.g. /api/items/{id}), then verifies IDs flow from list to detail.
 */
function detectListDetailPairs(apiRequests: ParsedApiRequest[]): WorkflowPattern[] {
  const patterns: WorkflowPattern[] = [];

  // Group by normalized path
  const byNormalized = new Map<string, ParsedApiRequest[]>();
  for (const req of apiRequests) {
    const norm = `${req.method} ${normalizePath(req.path)}`;
    const arr = byNormalized.get(norm) || [];
    arr.push(req);
    byNormalized.set(norm, arr);
  }

  const normalizedKeys = [...byNormalized.keys()];

  for (const listKey of normalizedKeys) {
    if (!listKey.startsWith('GET ')) continue;
    const listPath = listKey.slice(4); // remove "GET "

    for (const detailKey of normalizedKeys) {
      if (!detailKey.startsWith('GET ')) continue;
      if (listKey === detailKey) continue;
      const detailPath = detailKey.slice(4);

      // Check if detail path is the list path + /{param}
      const hasIdSuffix =
        detailPath.startsWith(listPath) &&
        /^\/{(?:id|uuid|token)}$/.test(detailPath.slice(listPath.length));

      if (!hasIdSuffix) continue;

      // Try to verify: extract IDs from list response, check if they appear in detail URLs
      const listReqs = byNormalized.get(listKey) ?? [];
      const detailReqs = byNormalized.get(detailKey) ?? [];

      const { fieldPath, confidence } = verifyIdFlow(listReqs, detailReqs);

      patterns.push({
        type: 'list-detail',
        description: `${listKey} (list) -> ${detailKey} (detail)`,
        confidence,
        details: {
          listEndpoint: listKey,
          detailEndpoint: detailKey,
          variableField: fieldPath,
          listCallCount: listReqs.length,
          detailCallCount: detailReqs.length,
        },
      });
    }
  }

  return patterns;
}

/**
 * Verify that IDs from list response bodies appear in detail request URLs.
 */
function verifyIdFlow(
  listReqs: ParsedApiRequest[],
  detailReqs: ParsedApiRequest[],
): { fieldPath: string; confidence: 'high' | 'medium' | 'low' } {
  // Extract ID-like values from list responses
  const listIds = new Set<string>();
  let fieldPath = '';

  for (const req of listReqs) {
    if (!req.responseBody) continue;
    try {
      const body = JSON.parse(req.responseBody);
      const ids = extractIdValues(body, '');
      for (const { value, path } of ids) {
        listIds.add(String(value));
        if (!fieldPath) fieldPath = path;
      }
    } catch {
      // Response might be truncated/schema — skip verification
    }
  }

  if (listIds.size === 0) {
    return { fieldPath: '(unknown)', confidence: 'medium' };
  }

  // Check how many detail URLs contain a list ID
  let matchCount = 0;
  for (const req of detailReqs) {
    const url = req.url;
    for (const id of listIds) {
      if (url.includes(id)) {
        matchCount++;
        break;
      }
    }
  }

  if (detailReqs.length === 0) {
    return { fieldPath, confidence: 'medium' };
  }

  const matchRatio = matchCount / detailReqs.length;
  const confidence = matchRatio > 0.5 ? 'high' : matchRatio > 0 ? 'medium' : 'low';

  return { fieldPath: fieldPath || '(unknown)', confidence };
}

/**
 * Extract ID-like values from a parsed JSON body.
 */
function extractIdValues(
  value: unknown,
  currentPath: string,
  results: { value: string | number; path: string }[] = [],
  depth = 0,
): { value: string | number; path: string }[] {
  if (depth > 4) return results;

  if (Array.isArray(value)) {
    // For arrays, check first few items
    for (const [i, item] of value.slice(0, 5).entries()) {
      extractIdValues(item, `${currentPath}[${i}]`, results, depth + 1);
    }
    return results;
  }

  if (value && typeof value === 'object') {
    for (const [key, val] of Object.entries(value as Record<string, unknown>)) {
      const path = currentPath ? `${currentPath}.${key}` : key;

      // Check if this field name is an ID field
      if (ID_FIELD_NAMES.has(key) && (typeof val === 'string' || typeof val === 'number')) {
        results.push({ value: val, path: path.replace(/\[\d+\]/, '[]') });
      }

      // Recurse into objects
      if (typeof val === 'object' && val !== null) {
        extractIdValues(val, path, results, depth + 1);
      }
    }
  }

  return results;
}

/**
 * Detect pagination patterns.
 * Looks for endpoint groups with >1 request where a query param changes.
 */
function detectPagination(apiRequests: ParsedApiRequest[]): WorkflowPattern[] {
  const patterns: WorkflowPattern[] = [];

  // Group by normalized path (without query)
  const byPath = new Map<string, ParsedApiRequest[]>();
  for (const req of apiRequests) {
    const norm = `${req.method} ${normalizePath(req.path)}`;
    const arr = byPath.get(norm) || [];
    arr.push(req);
    byPath.set(norm, arr);
  }

  for (const [pathKey, reqs] of byPath) {
    if (reqs.length < 2) continue;

    // Find params that change across requests
    const allParams = new Map<string, Set<string>>();
    for (const req of reqs) {
      for (const [k, v] of Object.entries(req.queryParams ?? {})) {
        if (!allParams.has(k)) allParams.set(k, new Set());
        allParams.get(k)!.add(v);
      }
    }

    const changingPaginationParams: string[] = [];
    for (const [param, values] of allParams) {
      if (values.size > 1 && PAGINATION_PARAMS.has(param.toLowerCase())) {
        changingPaginationParams.push(param);
      }
    }

    if (changingPaginationParams.length > 0) {
      // Determine pagination type
      const paginationType = changingPaginationParams.some(p =>
        ['cursor', 'after', 'before', 'nextToken', 'pageToken', 'page_token', 'continuation', 'startCursor', 'start_cursor'].includes(p),
      )
        ? 'cursor-based'
        : 'offset-based';

      patterns.push({
        type: 'pagination',
        description: `${pathKey}: ${paginationType} (param: "${changingPaginationParams.join(', ')}")`,
        confidence: 'high',
        details: {
          endpoint: pathKey,
          paginationType,
          paginationParams: changingPaginationParams,
          callCount: reqs.length,
        },
      });
    }
  }

  return patterns;
}

/**
 * Detect variable flow — IDs from response bodies appearing in subsequent request URLs.
 */
function detectVariableFlow(apiRequests: ParsedApiRequest[]): WorkflowPattern[] {
  const patterns: WorkflowPattern[] = [];

  // Sort by timestamp
  const sorted = [...apiRequests].sort(
    (a, b) => new Date(a.timestamp ?? '').getTime() - new Date(b.timestamp ?? '').getTime(),
  );

  // For each request with a response body, extract ID values and check if they appear
  // in any subsequent request URLs
  const seenFlows = new Set<string>();

  for (let i = 0; i < sorted.length && i < 50; i++) {
    const source = sorted[i];
    if (!source.responseBody) continue;

    let ids: { value: string | number; path: string }[] = [];
    try {
      const body = JSON.parse(source.responseBody);
      ids = extractIdValues(body, '');
    } catch {
      continue;
    }

    if (ids.length === 0) continue;

    const sourceNorm = `${source.method} ${normalizePath(source.path)}`;

    for (let j = i + 1; j < sorted.length && j < i + 30; j++) {
      const target = sorted[j];
      const targetNorm = `${target.method} ${normalizePath(target.path)}`;

      // Skip same endpoint group
      if (sourceNorm === targetNorm) continue;

      for (const { value, path: fieldPath } of ids) {
        const strVal = String(value);
        if (strVal.length < 3) continue; // skip very short IDs

        if (target.url.includes(strVal)) {
          const flowKey = `${sourceNorm} -> ${targetNorm} via ${fieldPath}`;
          if (!seenFlows.has(flowKey)) {
            seenFlows.add(flowKey);
            patterns.push({
              type: 'variable-flow',
              description: `ID from ${sourceNorm} field "${fieldPath}" flows into ${targetNorm}`,
              confidence: 'medium',
              details: {
                sourceEndpoint: sourceNorm,
                targetEndpoint: targetNorm,
                field: fieldPath,
                sampleValue: strVal.slice(0, 30),
              },
            });
          }
          break; // one match per target is enough
        }
      }
    }
  }

  return patterns;
}

/**
 * Render workflow patterns as a readable string for the LLM prompt.
 */
function renderWorkflowSummary(patterns: WorkflowPattern[]): string {
  if (patterns.length === 0) {
    return 'No workflow patterns detected.';
  }

  const lines: string[] = ['## Detected Workflow Patterns'];

  const listDetail = patterns.filter(p => p.type === 'list-detail');
  const pagination = patterns.filter(p => p.type === 'pagination');
  const varFlow = patterns.filter(p => p.type === 'variable-flow');

  if (listDetail.length > 0) {
    lines.push('');
    lines.push('### List -> Detail');
    for (const p of listDetail) {
      lines.push(`- ${p.description}`);
      if (p.details.variableField) {
        lines.push(`  Variable: flows from response field "${p.details.variableField}" into detail URL`);
      }
      lines.push(`  Confidence: ${p.confidence}`);
    }
  }

  if (pagination.length > 0) {
    lines.push('');
    lines.push('### Pagination');
    for (const p of pagination) {
      lines.push(`- ${p.description}`);
    }
  }

  if (varFlow.length > 0) {
    lines.push('');
    lines.push('### Variable Flow (IDs flowing between endpoints)');
    for (const p of varFlow.slice(0, 10)) {
      lines.push(`- ${p.description}`);
      lines.push(`  Sample value: "${p.details.sampleValue}"`);
    }
    if (varFlow.length > 10) {
      lines.push(`  ... and ${varFlow.length - 10} more`);
    }
  }

  return lines.join('\n');
}
