/**
 * Action-API Correlator — Link user actions to the API calls they triggered.
 *
 * For each action, finds API requests that fired within a 2-second window.
 */

import type { ActionWithIntent, ParsedApiRequest, ActionApiTimeline, CorrelatedAction, TriggeredApi, UserAction } from '../types.js';

const CORRELATION_WINDOW_MS = 2000;

/**
 * Convert explorer ActionWithIntent to UserAction format for correlation.
 */
function toUserAction(action: ActionWithIntent, baseUrl: string): UserAction {
  return {
    type: action.action,
    timestamp: new Date(action.timestamp).toISOString(),
    url: action.url || baseUrl,
    selector: action.ref ? `@${action.ref}` : undefined,
    value: action.text,
    key: action.key,
    description: action.intention,
  };
}

/**
 * Correlate explorer actions with API requests from HAR.
 */
export function correlateActionsWithApis(
  actions: ActionWithIntent[],
  apiRequests: ParsedApiRequest[],
  baseUrl: string,
): ActionApiTimeline {
  const userActions = actions.map(a => toUserAction(a, baseUrl));

  const sortedActions = [...userActions].sort(
    (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
  );
  const sortedApis = [...apiRequests].sort(
    (a, b) => new Date(a.timestamp ?? '').getTime() - new Date(b.timestamp ?? '').getTime(),
  );

  const claimedApiIndices = new Set<number>();
  const correlatedActions: CorrelatedAction[] = [];

  for (const [actionIdx, action] of sortedActions.entries()) {
    const actionTime = new Date(action.timestamp).getTime();
    if (isNaN(actionTime)) continue;

    const triggeredApis: TriggeredApi[] = [];

    for (const [apiIdx, api] of sortedApis.entries()) {
      if (claimedApiIndices.has(apiIdx)) continue;

      const apiTime = new Date(api.timestamp ?? '').getTime();
      if (isNaN(apiTime)) continue;

      const delayMs = apiTime - actionTime;
      if (delayMs >= 0 && delayMs <= CORRELATION_WINDOW_MS) {
        claimedApiIndices.add(apiIdx);
        triggeredApis.push({
          method: api.method,
          url: api.url,
          path: api.path,
          status: api.status,
          delayMs,
          responseContentType: api.responseContentType,
        });
      }
    }

    correlatedActions.push({ index: actionIdx + 1, action, triggeredApis });
  }

  const uncorrelatedApis = sortedApis.filter((_, i) => !claimedApiIndices.has(i));
  return { correlatedActions, uncorrelatedApis };
}

/**
 * Render the correlated timeline as a readable string.
 */
export function renderTimeline(timeline: ActionApiTimeline): string {
  const lines: string[] = [];

  for (const ca of timeline.correlatedActions) {
    const a = ca.action;
    let actionDesc: string;
    switch (a.type) {
      case 'navigate': actionDesc = `Navigate to: ${a.url}`; break;
      case 'click': actionDesc = `Click: ${a.selector} ("${a.description?.slice(0, 50) ?? ''}")`; break;
      case 'type': actionDesc = `Type "${a.value}" into: ${a.selector}`; break;
      case 'press': actionDesc = `Press ${a.key} on: ${a.selector}`; break;
      default: actionDesc = `${a.type}: ${a.description || a.selector || ''}`; break;
    }

    lines.push(`${ca.index}. ${actionDesc}`);
    for (const api of ca.triggeredApis) {
      lines.push(`   -> ${api.method} ${api.path.split('?')[0]} (${api.status}, ${api.delayMs}ms later)`);
    }
    if (ca.triggeredApis.length === 0) lines.push(`   (no API calls triggered)`);
  }

  if (timeline.uncorrelatedApis.length > 0) {
    lines.push('', `Background API calls (${timeline.uncorrelatedApis.length} total):`);
    for (const api of timeline.uncorrelatedApis.slice(0, 10)) {
      lines.push(`  - ${api.method} ${api.path.split('?')[0]} (${api.status})`);
    }
    if (timeline.uncorrelatedApis.length > 10) {
      lines.push(`  ... and ${timeline.uncorrelatedApis.length - 10} more`);
    }
  }

  return lines.join('\n');
}
