/**
 * Enhanced snapshot with element refs for deterministic element selection.
 *
 * Copied from vercel-labs/agent-browser (MIT License) and adapted for data-agent.
 * Generates accessibility snapshots with embedded refs that can be used to
 * interact with elements without re-querying the DOM.
 *
 * Example output:
 *   - heading "Example Domain" [ref=e1] [level=1]
 *   - button "Submit" [ref=e2]
 *   - textbox "Email" [ref=e3]
 */

import type { Page } from 'playwright';

export interface RefMap {
  [ref: string]: {
    selector: string;
    role: string;
    name?: string;
    nth?: number;
  };
}

export interface EnhancedSnapshot {
  tree: string;
  refs: RefMap;
}

export interface SnapshotOptions {
  /** Only include interactive elements. */
  interactive?: boolean;
  /** Include cursor-interactive elements (cursor:pointer, onclick). */
  cursor?: boolean;
  /** Maximum depth of tree. */
  maxDepth?: number;
  /** Remove structural elements without meaningful content. */
  compact?: boolean;
  /** CSS selector to scope the snapshot. */
  selector?: string;
}

let refCounter = 0;

export function resetRefs(): void {
  refCounter = 0;
}

function nextRef(): string {
  return `e${++refCounter}`;
}

const INTERACTIVE_ROLES = new Set([
  'button', 'link', 'textbox', 'checkbox', 'radio', 'combobox', 'listbox',
  'menuitem', 'menuitemcheckbox', 'menuitemradio', 'option', 'searchbox',
  'slider', 'spinbutton', 'switch', 'tab', 'treeitem',
]);

const CONTENT_ROLES = new Set([
  'heading', 'cell', 'gridcell', 'columnheader', 'rowheader',
  'listitem', 'article', 'region', 'main', 'navigation',
]);

const STRUCTURAL_ROLES = new Set([
  'generic', 'group', 'list', 'table', 'row', 'rowgroup', 'grid',
  'treegrid', 'menu', 'menubar', 'toolbar', 'tablist', 'tree',
  'directory', 'document', 'application', 'presentation', 'none',
]);

function buildSelector(role: string, name?: string): string {
  if (name) {
    const escapedName = name.replace(/"/g, '\\"');
    return `getByRole('${role}', { name: "${escapedName}", exact: true })`;
  }
  return `getByRole('${role}')`;
}

async function findCursorInteractiveElements(
  page: Page,
  selector?: string,
): Promise<Array<{
  selector: string;
  text: string;
  tagName: string;
  hasOnClick: boolean;
  hasCursorPointer: boolean;
  hasTabIndex: boolean;
}>> {
  const rootSelector = selector || 'body';

  const scriptBody = `(rootSel) => {
    const results = [];
    const interactiveRoles = new Set([
      'button', 'link', 'textbox', 'checkbox', 'radio', 'combobox', 'listbox',
      'menuitem', 'menuitemcheckbox', 'menuitemradio', 'option', 'searchbox',
      'slider', 'spinbutton', 'switch', 'tab', 'treeitem'
    ]);
    const interactiveTags = new Set([
      'a', 'button', 'input', 'select', 'textarea', 'details', 'summary'
    ]);
    const root = document.querySelector(rootSel) || document.body;
    const allElements = root.querySelectorAll('*');

    const buildSelector = (el) => {
      const testId = el.getAttribute('data-testid');
      if (testId) return '[data-testid="' + testId + '"]';
      if (el.id) return '#' + CSS.escape(el.id);
      const path = [];
      let current = el;
      while (current && current !== document.body) {
        let sel = current.tagName.toLowerCase();
        const classes = Array.from(current.classList).filter(c => c.trim());
        if (classes.length > 0) sel += '.' + CSS.escape(classes[0]);
        const parent = current.parentElement;
        if (parent) {
          const siblings = Array.from(parent.children);
          const matching = siblings.filter(s => {
            if (s.tagName !== current.tagName) return false;
            if (classes.length > 0 && !s.classList.contains(classes[0])) return false;
            return true;
          });
          if (matching.length > 1) {
            const idx = matching.indexOf(current) + 1;
            sel += ':nth-of-type(' + idx + ')';
          }
        }
        path.unshift(sel);
        current = current.parentElement;
        if (path.length >= 3) break;
      }
      return path.join(' > ');
    };

    for (const el of allElements) {
      const tagName = el.tagName.toLowerCase();
      if (interactiveTags.has(tagName)) continue;
      const role = el.getAttribute('role');
      if (role && interactiveRoles.has(role.toLowerCase())) continue;
      const computedStyle = getComputedStyle(el);
      const hasCursorPointer = computedStyle.cursor === 'pointer';
      const hasOnClick = el.hasAttribute('onclick') || el.onclick !== null;
      const tabIndex = el.getAttribute('tabindex');
      const hasTabIndex = tabIndex !== null && tabIndex !== '-1';
      if (!hasCursorPointer && !hasOnClick && !hasTabIndex) continue;
      const text = (el.textContent || '').trim().slice(0, 100);
      if (!text) continue;
      const rect = el.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0) continue;
      results.push({
        selector: buildSelector(el),
        text,
        tagName,
        hasOnClick,
        hasCursorPointer,
        hasTabIndex
      });
    }
    return results;
  }`;

  // eslint-disable-next-line @typescript-eslint/no-implied-eval
  const fn = new Function('return ' + scriptBody)();
  return page.evaluate(fn, rootSelector);
}

export async function getEnhancedSnapshot(
  page: Page,
  options: SnapshotOptions = {},
): Promise<EnhancedSnapshot> {
  resetRefs();
  const refs: RefMap = {};

  const locator = options.selector ? page.locator(options.selector) : page.locator(':root');
  const ariaTree = await locator.ariaSnapshot();

  if (!ariaTree) {
    return { tree: '(empty)', refs: {} };
  }

  const enhancedTree = processAriaTree(ariaTree, refs, options);

  if (options.cursor) {
    const cursorElements = await findCursorInteractiveElements(page, options.selector);
    const existingTexts = new Set(Object.values(refs).map(r => r.name?.toLowerCase()));

    const additionalLines: string[] = [];
    for (const el of cursorElements) {
      if (existingTexts.has(el.text.toLowerCase())) continue;

      const ref = nextRef();
      const role = el.hasCursorPointer || el.hasOnClick ? 'clickable' : 'focusable';

      refs[ref] = { selector: el.selector, role, name: el.text };

      const hints: string[] = [];
      if (el.hasCursorPointer) hints.push('cursor:pointer');
      if (el.hasOnClick) hints.push('onclick');
      if (el.hasTabIndex) hints.push('tabindex');

      additionalLines.push(`- ${role} "${el.text}" [ref=${ref}] [${hints.join(', ')}]`);
    }

    if (additionalLines.length > 0) {
      const separator = enhancedTree === '(no interactive elements)' ? '' : '\n# Cursor-interactive elements:\n';
      const base = enhancedTree === '(no interactive elements)' ? '' : enhancedTree;
      return { tree: base + separator + additionalLines.join('\n'), refs };
    }
  }

  return { tree: enhancedTree, refs };
}

interface RoleNameTracker {
  counts: Map<string, number>;
  refsByKey: Map<string, string[]>;
  getKey(role: string, name?: string): string;
  getNextIndex(role: string, name?: string): number;
  trackRef(role: string, name: string | undefined, ref: string): void;
  getDuplicateKeys(): Set<string>;
}

function createRoleNameTracker(): RoleNameTracker {
  const counts = new Map<string, number>();
  const refsByKey = new Map<string, string[]>();
  return {
    counts,
    refsByKey,
    getKey(role: string, name?: string): string {
      return `${role}:${name ?? ''}`;
    },
    getNextIndex(role: string, name?: string): number {
      const key = this.getKey(role, name);
      const current = counts.get(key) ?? 0;
      counts.set(key, current + 1);
      return current;
    },
    trackRef(role: string, name: string | undefined, ref: string): void {
      const key = this.getKey(role, name);
      const existingRefs = refsByKey.get(key) ?? [];
      existingRefs.push(ref);
      refsByKey.set(key, existingRefs);
    },
    getDuplicateKeys(): Set<string> {
      const duplicates = new Set<string>();
      for (const [key, keyRefs] of refsByKey) {
        if (keyRefs.length > 1) duplicates.add(key);
      }
      return duplicates;
    },
  };
}

function processAriaTree(ariaTree: string, refs: RefMap, options: SnapshotOptions): string {
  const lines = ariaTree.split('\n');
  const result: string[] = [];
  const tracker = createRoleNameTracker();

  if (options.interactive) {
    for (const line of lines) {
      const match = line.match(/^(\s*-\s*)(\w+)(?:\s+"([^"]*)")?(.*)$/);
      if (!match) continue;

      const [, , role, name, suffix] = match;
      const roleLower = role.toLowerCase();

      if (INTERACTIVE_ROLES.has(roleLower)) {
        const ref = nextRef();
        const nth = tracker.getNextIndex(roleLower, name);
        tracker.trackRef(roleLower, name, ref);
        refs[ref] = { selector: buildSelector(roleLower, name), role: roleLower, name, nth };

        let enhanced = `- ${role}`;
        if (name) enhanced += ` "${name}"`;
        enhanced += ` [ref=${ref}]`;
        if (nth > 0) enhanced += ` [nth=${nth}]`;
        if (suffix && suffix.includes('[')) enhanced += suffix;

        result.push(enhanced);
      }
    }

    removeNthFromNonDuplicates(refs, tracker);
    return result.join('\n') || '(no interactive elements)';
  }

  for (const line of lines) {
    const processed = processLine(line, refs, options, tracker);
    if (processed !== null) result.push(processed);
  }

  removeNthFromNonDuplicates(refs, tracker);

  if (options.compact) return compactTree(result.join('\n'));
  return result.join('\n');
}

function removeNthFromNonDuplicates(refs: RefMap, tracker: RoleNameTracker): void {
  const duplicateKeys = tracker.getDuplicateKeys();
  for (const [, data] of Object.entries(refs)) {
    const key = tracker.getKey(data.role, data.name);
    if (!duplicateKeys.has(key)) delete data.nth;
  }
}

function getIndentLevel(line: string): number {
  const match = line.match(/^(\s*)/);
  return match ? Math.floor(match[1].length / 2) : 0;
}

function processLine(
  line: string,
  refs: RefMap,
  options: SnapshotOptions,
  tracker: RoleNameTracker,
): string | null {
  const depth = getIndentLevel(line);

  if (options.maxDepth !== undefined && depth > options.maxDepth) return null;

  const match = line.match(/^(\s*-\s*)(\w+)(?:\s+"([^"]*)")?(.*)$/);
  if (!match) {
    if (options.interactive) return null;
    return line;
  }

  const [, prefix, role, name, suffix] = match;
  const roleLower = role.toLowerCase();

  if (role.startsWith('/')) return line;

  const isInteractive = INTERACTIVE_ROLES.has(roleLower);
  const isContent = CONTENT_ROLES.has(roleLower);
  const isStructural = STRUCTURAL_ROLES.has(roleLower);

  if (options.interactive && !isInteractive) return null;
  if (options.compact && isStructural && !name) return null;

  const shouldHaveRef = isInteractive || (isContent && name);

  if (shouldHaveRef) {
    const ref = nextRef();
    const nth = tracker.getNextIndex(roleLower, name);
    tracker.trackRef(roleLower, name, ref);
    refs[ref] = { selector: buildSelector(roleLower, name), role: roleLower, name, nth };

    let enhanced = `${prefix}${role}`;
    if (name) enhanced += ` "${name}"`;
    enhanced += ` [ref=${ref}]`;
    if (nth > 0) enhanced += ` [nth=${nth}]`;
    if (suffix) enhanced += suffix;

    return enhanced;
  }

  return line;
}

function compactTree(tree: string): string {
  const lines = tree.split('\n');
  const result: string[] = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    if (line.includes('[ref=')) { result.push(line); continue; }
    if (line.includes(':') && !line.endsWith(':')) { result.push(line); continue; }

    const currentIndent = getIndentLevel(line);
    let hasRelevantChildren = false;

    for (let j = i + 1; j < lines.length; j++) {
      const childIndent = getIndentLevel(lines[j]);
      if (childIndent <= currentIndent) break;
      if (lines[j].includes('[ref=')) { hasRelevantChildren = true; break; }
    }

    if (hasRelevantChildren) result.push(line);
  }

  return result.join('\n');
}

export function parseRef(arg: string): string | null {
  if (arg.startsWith('@')) return arg.slice(1);
  if (arg.startsWith('ref=')) return arg.slice(4);
  if (/^e\d+$/.test(arg)) return arg;
  return null;
}

export function getSnapshotStats(
  tree: string,
  refs: RefMap,
): { lines: number; chars: number; tokens: number; refs: number; interactive: number } {
  const interactive = Object.values(refs).filter(r => INTERACTIVE_ROLES.has(r.role)).length;
  return {
    lines: tree.split('\n').length,
    chars: tree.length,
    tokens: Math.ceil(tree.length / 4),
    refs: Object.keys(refs).length,
    interactive,
  };
}
