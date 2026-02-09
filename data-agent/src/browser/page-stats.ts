/**
 * Page statistics + blocked-page detection.
 *
 * Collects lightweight page metrics (element counts, link/iframe/shadow counts)
 * and uses heuristics to detect blocked or interstitial pages early.
 *
 * Inspired by browser-use's page statistics injection:
 * @see https://github.com/browser-use/browser-use/blob/main/browser_use/agent/prompts.py
 * AgentMessagePrompt._extract_page_statistics() — page stats for context injection
 *
 * Block detection heuristic inspired by browser-use system prompt anti-loop rules:
 * @see https://github.com/browser-use/browser-use/blob/main/browser_use/agent/system_prompts/system_prompt.md
 */

import type { Page } from 'playwright';

export interface PageStats {
  /** Total DOM element count. */
  totalElements: number;
  /** Interactive elements (buttons, links, inputs, selects, textareas). */
  interactiveElements: number;
  /** Number of <a> links. */
  links: number;
  /** Number of <iframe> elements. */
  iframes: number;
  /** Number of <img> elements. */
  images: number;
  /** Number of shadow roots (open). */
  shadowRoots: number;
  /** Number of form fields (input, select, textarea). */
  formFields: number;
  /** Visible body text length (characters). */
  bodyTextLength: number;
  /** Page title. */
  pageTitle: string;
  /** Current URL. */
  currentUrl: string;
  /** Whether the page appears to be a block/interstitial page. */
  isLikelyBlocked: boolean;
  /** Reason for block detection (if any). */
  blockReason?: string;
}

/** Title patterns that indicate a blocked or error page. */
const BLOCK_TITLE_PATTERNS = [
  /access\s*denied/i,
  /403\s*forbidden/i,
  /blocked/i,
  /captcha/i,
  /challenge/i,
  /just\s*a\s*moment/i,        // Cloudflare
  /attention\s*required/i,     // Cloudflare
  /please\s*wait/i,
  /checking\s*your\s*browser/i,
  /security\s*check/i,
  /are\s*you\s*a\s*human/i,
  /bot\s*detection/i,
  /rate\s*limit/i,
  /too\s*many\s*requests/i,
  /error\s*\d{3}/i,
  /page\s*not\s*found/i,
  /404/,
  /500\s*internal/i,
  /503\s*service/i,
];

/** Body text snippets that indicate block pages. */
const BLOCK_BODY_PATTERNS = [
  /enable\s*javascript.*cookies/i,
  /ray\s*id/i,                 // Cloudflare
  /cf-browser-verification/i,
  /recaptcha/i,
  /hcaptcha/i,
  /please\s*verify.*human/i,
];

/**
 * Collect page statistics and detect blocked/interstitial state.
 */
export async function getPageStats(page: Page): Promise<PageStats> {
  const stats = await page.evaluate(() => {
    const all = document.querySelectorAll('*');
    const interactive = document.querySelectorAll(
      'button, a, input, select, textarea, [role="button"], [role="link"], [tabindex]',
    );
    const links = document.querySelectorAll('a');
    const iframes = document.querySelectorAll('iframe');
    const images = document.querySelectorAll('img');
    const formFields = document.querySelectorAll('input, select, textarea');

    // Count open shadow roots
    let shadowRoots = 0;
    for (const el of all) {
      if (el.shadowRoot) shadowRoots++;
    }

    const bodyText = (document.body?.innerText || '').trim();

    return {
      totalElements: all.length,
      interactiveElements: interactive.length,
      links: links.length,
      iframes: iframes.length,
      images: images.length,
      shadowRoots,
      formFields: formFields.length,
      bodyTextLength: bodyText.length,
      // Send first 500 chars for block-pattern matching
      bodyTextSnippet: bodyText.slice(0, 500),
      pageTitle: document.title || '',
    };
  });

  const currentUrl = page.url();

  // Blocked-page detection heuristics
  let isLikelyBlocked = false;
  let blockReason: string | undefined;

  // Check title patterns
  for (const pattern of BLOCK_TITLE_PATTERNS) {
    if (pattern.test(stats.pageTitle)) {
      isLikelyBlocked = true;
      blockReason = `Page title matches block pattern: "${stats.pageTitle}"`;
      break;
    }
  }

  // Check body text patterns
  if (!isLikelyBlocked) {
    for (const pattern of BLOCK_BODY_PATTERNS) {
      if (pattern.test(stats.bodyTextSnippet)) {
        isLikelyBlocked = true;
        blockReason = `Page body matches block pattern: ${pattern}`;
        break;
      }
    }
  }

  // Extremely sparse page heuristic: < 3 interactive elements AND < 30 total
  if (!isLikelyBlocked && stats.interactiveElements < 3 && stats.totalElements < 30) {
    isLikelyBlocked = true;
    blockReason = `Sparse page: ${stats.interactiveElements} interactive, ${stats.totalElements} total elements`;
  }

  // Very short body text on a non-trivial URL
  if (!isLikelyBlocked && stats.bodyTextLength < 200 && stats.totalElements < 50) {
    isLikelyBlocked = true;
    blockReason = `Minimal content: ${stats.bodyTextLength} chars body text, ${stats.totalElements} elements`;
  }

  return {
    totalElements: stats.totalElements,
    interactiveElements: stats.interactiveElements,
    links: stats.links,
    iframes: stats.iframes,
    images: stats.images,
    shadowRoots: stats.shadowRoots,
    formFields: stats.formFields,
    bodyTextLength: stats.bodyTextLength,
    pageTitle: stats.pageTitle,
    currentUrl,
    isLikelyBlocked,
    blockReason,
  };
}

/**
 * Format page stats as a compact string for LLM context injection.
 */
export function formatPageStats(stats: PageStats): string {
  const parts = [
    `elements=${stats.totalElements}`,
    `interactive=${stats.interactiveElements}`,
    `links=${stats.links}`,
    `forms=${stats.formFields}`,
    `iframes=${stats.iframes}`,
    `shadows=${stats.shadowRoots}`,
    `bodyLen=${stats.bodyTextLength}`,
  ];
  if (stats.isLikelyBlocked) {
    parts.push(`BLOCKED(${stats.blockReason})`);
  }
  return `[page-stats: ${parts.join(', ')}]`;
}
