/**
 * Action-API Correlator — Link user actions to the API calls they triggered.
 *
 * For each recorded action, finds API requests that fired within a short
 * time window after it, giving the LLM causal evidence of which clicks
 * trigger which endpoints.
 */

import type { UserAction, ParsedApiRequest, ActionApiTimeline, CorrelatedAction, TriggeredApi } from './types.js';

/** Maximum delay (ms) between an action and an API call to consider them correlated. */
const CORRELATION_WINDOW_MS = 2000;

/**
 * Correlate user actions with the API requests they triggered.
 *
 * For each action, finds API requests that started within CORRELATION_WINDOW_MS
 * after the action's timestamp. Each API request is assigned to at most one action
 * (the closest preceding one).
 */
export function correlateActionsWithApis(
  actions: UserAction[],
  apiRequests: ParsedApiRequest[],
): ActionApiTimeline {
  // Sort both by timestamp
  const sortedActions = [...actions].sort(
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

      // API must come after the action and within the window
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

    correlatedActions.push({
      index: actionIdx + 1,
      action,
      triggeredApis,
    });
  }

  // Uncorrelated APIs = those not claimed by any action (background polling, preloads, etc.)
  const uncorrelatedApis = sortedApis.filter((_, i) => !claimedApiIndices.has(i));

  return { correlatedActions, uncorrelatedApis };
}

/**
 * Render the correlated timeline as a readable string for the LLM prompt.
 */
export function renderTimeline(timeline: ActionApiTimeline): string {
  const lines: string[] = [];

  for (const ca of timeline.correlatedActions) {
    const a = ca.action;
    let actionDesc: string;
    switch (a.type) {
      case 'navigate':
        actionDesc = `Navigate to: ${a.url}`;
        break;
      case 'click':
        actionDesc = `Click: ${a.selector} (${a.tagName}: "${a.text?.slice(0, 50) ?? ''}")`;
        break;
      case 'type':
        actionDesc = `Type "${a.value}" into: ${a.selector}`;
        break;
      case 'press':
        actionDesc = `Press ${a.key} on: ${a.selector}`;
        break;
      default:
        actionDesc = `${a.type}: ${a.description || a.selector || ''}`;
    }

    lines.push(`${ca.index}. ${actionDesc}`);

    for (const api of ca.triggeredApis) {
      lines.push(`   -> ${api.method} ${api.path.split('?')[0]} (${api.status}, ${api.delayMs}ms later)`);
    }

    if (ca.triggeredApis.length === 0) {
      lines.push(`   (no API calls triggered)`);
    }
  }

  if (timeline.uncorrelatedApis.length > 0) {
    lines.push('');
    lines.push(`Background API calls (${timeline.uncorrelatedApis.length} total, not triggered by user actions):`);
    // Show up to 10 background APIs
    for (const api of timeline.uncorrelatedApis.slice(0, 10)) {
      const path = api.path.split('?')[0];
      lines.push(`  - ${api.method} ${path} (${api.status})`);
    }
    if (timeline.uncorrelatedApis.length > 10) {
      lines.push(`  ... and ${timeline.uncorrelatedApis.length - 10} more`);
    }
  }

  return lines.join('\n');
}
