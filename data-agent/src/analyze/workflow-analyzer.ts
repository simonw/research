/**
 * Workflow Analyzer — Detect list→detail patterns, pagination, and variable flow.
 */

import type { ParsedApiRequest, WorkflowAnalysis, WorkflowPattern } from '../types.js';
import { normalizePath } from './ir-builder.js';

const ID_FIELD_NAMES = new Set([
  'id', '_id', 'uuid', 'slug', 'key', 'pk',
  'itemId', 'item_id', 'postId', 'post_id',
  'userId', 'user_id', 'threadId', 'thread_id',
  'conversationId', 'conversation_id', 'messageId', 'message_id',
  'projectId', 'project_id', 'documentId', 'document_id',
]);

const PAGINATION_PARAMS = new Set([
  'page', 'offset', 'cursor', 'after', 'before',
  'skip', 'start', 'from', 'next', 'nextToken',
  'continuation', 'pageToken', 'page_token',
  'startCursor', 'start_cursor', 'endCursor', 'end_cursor',
]);

export function analyzeWorkflow(apiRequests: ParsedApiRequest[]): WorkflowAnalysis {
  const patterns: WorkflowPattern[] = [];
  patterns.push(...detectListDetailPairs(apiRequests));
  patterns.push(...detectPagination(apiRequests));
  patterns.push(...detectVariableFlow(apiRequests));
  const summary = renderWorkflowSummary(patterns);
  return { patterns, summary };
}

function detectListDetailPairs(apiRequests: ParsedApiRequest[]): WorkflowPattern[] {
  const patterns: WorkflowPattern[] = [];
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
    const listPath = listKey.slice(4);

    for (const detailKey of normalizedKeys) {
      if (!detailKey.startsWith('GET ')) continue;
      if (listKey === detailKey) continue;
      const detailPath = detailKey.slice(4);

      const hasIdSuffix =
        detailPath.startsWith(listPath) &&
        /^\/{(?:id|uuid|token)}$/.test(detailPath.slice(listPath.length));

      if (!hasIdSuffix) continue;

      const listReqs = byNormalized.get(listKey) ?? [];
      const detailReqs = byNormalized.get(detailKey) ?? [];
      const { fieldPath, confidence } = verifyIdFlow(listReqs, detailReqs);

      patterns.push({
        type: 'list-detail',
        description: `${listKey} (list) -> ${detailKey} (detail)`,
        confidence,
        details: {
          listEndpoint: listKey, detailEndpoint: detailKey,
          variableField: fieldPath,
          listCallCount: listReqs.length, detailCallCount: detailReqs.length,
        },
      });
    }
  }

  return patterns;
}

function verifyIdFlow(
  listReqs: ParsedApiRequest[],
  detailReqs: ParsedApiRequest[],
): { fieldPath: string; confidence: 'high' | 'medium' | 'low' } {
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
    } catch { /* skip */ }
  }

  if (listIds.size === 0) return { fieldPath: '(unknown)', confidence: 'medium' };

  let matchCount = 0;
  for (const req of detailReqs) {
    for (const id of listIds) {
      if (req.url.includes(id)) { matchCount++; break; }
    }
  }

  if (detailReqs.length === 0) return { fieldPath, confidence: 'medium' };
  const matchRatio = matchCount / detailReqs.length;
  const confidence = matchRatio > 0.5 ? 'high' : matchRatio > 0 ? 'medium' : 'low';
  return { fieldPath: fieldPath || '(unknown)', confidence };
}

function extractIdValues(
  value: unknown,
  currentPath: string,
  results: { value: string | number; path: string }[] = [],
  depth = 0,
): { value: string | number; path: string }[] {
  if (depth > 4) return results;

  if (Array.isArray(value)) {
    for (const [i, item] of value.slice(0, 5).entries()) {
      extractIdValues(item, `${currentPath}[${i}]`, results, depth + 1);
    }
    return results;
  }

  if (value && typeof value === 'object') {
    for (const [key, val] of Object.entries(value as Record<string, unknown>)) {
      const path = currentPath ? `${currentPath}.${key}` : key;
      if (ID_FIELD_NAMES.has(key) && (typeof val === 'string' || typeof val === 'number')) {
        results.push({ value: val, path: path.replace(/\[\d+\]/, '[]') });
      }
      if (typeof val === 'object' && val !== null) {
        extractIdValues(val, path, results, depth + 1);
      }
    }
  }

  return results;
}

function detectPagination(apiRequests: ParsedApiRequest[]): WorkflowPattern[] {
  const patterns: WorkflowPattern[] = [];
  const byPath = new Map<string, ParsedApiRequest[]>();

  for (const req of apiRequests) {
    const norm = `${req.method} ${normalizePath(req.path)}`;
    const arr = byPath.get(norm) || [];
    arr.push(req);
    byPath.set(norm, arr);
  }

  for (const [pathKey, reqs] of byPath) {
    if (reqs.length < 2) continue;

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
      const paginationType = changingPaginationParams.some(p =>
        ['cursor', 'after', 'before', 'nextToken', 'pageToken', 'page_token', 'continuation', 'startCursor', 'start_cursor'].includes(p),
      ) ? 'cursor-based' : 'offset-based';

      patterns.push({
        type: 'pagination',
        description: `${pathKey}: ${paginationType} (param: "${changingPaginationParams.join(', ')}")`,
        confidence: 'high',
        details: {
          endpoint: pathKey, paginationType,
          paginationParams: changingPaginationParams, callCount: reqs.length,
        },
      });
    }
  }

  return patterns;
}

function detectVariableFlow(apiRequests: ParsedApiRequest[]): WorkflowPattern[] {
  const patterns: WorkflowPattern[] = [];
  const sorted = [...apiRequests].sort(
    (a, b) => new Date(a.timestamp ?? '').getTime() - new Date(b.timestamp ?? '').getTime(),
  );
  const seenFlows = new Set<string>();

  for (let i = 0; i < sorted.length && i < 50; i++) {
    const source = sorted[i];
    if (!source.responseBody) continue;

    let ids: { value: string | number; path: string }[] = [];
    try { ids = extractIdValues(JSON.parse(source.responseBody), ''); } catch { continue; }
    if (ids.length === 0) continue;

    const sourceNorm = `${source.method} ${normalizePath(source.path)}`;

    for (let j = i + 1; j < sorted.length && j < i + 30; j++) {
      const target = sorted[j];
      const targetNorm = `${target.method} ${normalizePath(target.path)}`;
      if (sourceNorm === targetNorm) continue;

      for (const { value, path: fieldPath } of ids) {
        const strVal = String(value);
        if (strVal.length < 3) continue;

        if (target.url.includes(strVal)) {
          const flowKey = `${sourceNorm} -> ${targetNorm} via ${fieldPath}`;
          if (!seenFlows.has(flowKey)) {
            seenFlows.add(flowKey);
            patterns.push({
              type: 'variable-flow',
              description: `ID from ${sourceNorm} field "${fieldPath}" flows into ${targetNorm}`,
              confidence: 'medium',
              details: {
                sourceEndpoint: sourceNorm, targetEndpoint: targetNorm,
                field: fieldPath, sampleValue: strVal.slice(0, 30),
              },
            });
          }
          break;
        }
      }
    }
  }

  return patterns;
}

function renderWorkflowSummary(patterns: WorkflowPattern[]): string {
  if (patterns.length === 0) return 'No workflow patterns detected.';

  const lines: string[] = ['## Detected Workflow Patterns'];
  const listDetail = patterns.filter(p => p.type === 'list-detail');
  const pagination = patterns.filter(p => p.type === 'pagination');
  const varFlow = patterns.filter(p => p.type === 'variable-flow');

  if (listDetail.length > 0) {
    lines.push('', '### List -> Detail');
    for (const p of listDetail) {
      lines.push(`- ${p.description}`);
      if (p.details.variableField) lines.push(`  Variable: flows from response field "${p.details.variableField}" into detail URL`);
      lines.push(`  Confidence: ${p.confidence}`);
    }
  }

  if (pagination.length > 0) {
    lines.push('', '### Pagination');
    for (const p of pagination) lines.push(`- ${p.description}`);
  }

  if (varFlow.length > 0) {
    lines.push('', '### Variable Flow (IDs flowing between endpoints)');
    for (const p of varFlow.slice(0, 10)) {
      lines.push(`- ${p.description}`);
      lines.push(`  Sample value: "${p.details.sampleValue}"`);
    }
    if (varFlow.length > 10) lines.push(`  ... and ${varFlow.length - 10} more`);
  }

  return lines.join('\n');
}
