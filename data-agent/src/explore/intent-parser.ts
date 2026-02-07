/**
 * Intent Parser — Extract domain, URL, task description from natural language.
 *
 * Converts "get my chatgpt conversations" into structured intent:
 * { domain: "chatgpt.com", url: "https://chatgpt.com", task: "get conversations", requiresAuth: true }
 */

import type { LlmProvider, ParsedIntent } from '../types.js';

/** Well-known domain mappings for common services. */
const DOMAIN_HINTS: Record<string, { domain: string; url: string; requiresAuth: boolean }> = {
  'chatgpt': { domain: 'chatgpt.com', url: 'https://chatgpt.com', requiresAuth: true },
  'openai': { domain: 'chatgpt.com', url: 'https://chatgpt.com', requiresAuth: true },
  'github': { domain: 'github.com', url: 'https://github.com', requiresAuth: true },
  'reddit': { domain: 'reddit.com', url: 'https://www.reddit.com', requiresAuth: false },
  'hacker news': { domain: 'news.ycombinator.com', url: 'https://news.ycombinator.com', requiresAuth: false },
  'hackernews': { domain: 'news.ycombinator.com', url: 'https://news.ycombinator.com', requiresAuth: false },
  'hn': { domain: 'news.ycombinator.com', url: 'https://news.ycombinator.com', requiresAuth: false },
  'twitter': { domain: 'x.com', url: 'https://x.com', requiresAuth: true },
  'x.com': { domain: 'x.com', url: 'https://x.com', requiresAuth: true },
  'youtube': { domain: 'youtube.com', url: 'https://www.youtube.com', requiresAuth: false },
  'linkedin': { domain: 'linkedin.com', url: 'https://www.linkedin.com', requiresAuth: true },
  'notion': { domain: 'notion.so', url: 'https://www.notion.so', requiresAuth: true },
  'slack': { domain: 'slack.com', url: 'https://slack.com', requiresAuth: true },
};

const INTENT_SYSTEM_PROMPT = `You are a task parser for a data extraction tool. Given a natural language description, extract:
1. The target website domain
2. A starting URL
3. A concise task description
4. Whether authentication/login is likely required (e.g. "my" data implies auth needed)

Respond with valid JSON only (no markdown fences):
{
  "domain": "example.com",
  "url": "https://example.com/starting-page",
  "task": "concise task description",
  "requiresAuth": true/false
}

Rules:
- If the user says "my" or "mine", requiresAuth is true
- If the site is public (like Hacker News top stories), requiresAuth is false
- The URL should be the best starting page for the task
- The task should be a clean, actionable description`;

/**
 * Parse natural language into structured intent.
 * Uses LLM when domain hints aren't sufficient.
 */
export async function parseIntent(description: string, llm: LlmProvider): Promise<ParsedIntent> {
  // Check for explicit URLs in the description
  const urlMatch = description.match(/https?:\/\/[^\s]+/);
  if (urlMatch) {
    const url = urlMatch[0];
    const domain = new URL(url).hostname;
    const task = description.replace(urlMatch[0], '').trim();
    const requiresAuth = /\bmy\b|\bmine\b|\baccount\b|\blogged in\b/i.test(description);
    return { domain, url, task: task || 'extract data', requiresAuth };
  }

  // Check domain hints
  const descLower = description.toLowerCase();
  for (const [keyword, hint] of Object.entries(DOMAIN_HINTS)) {
    if (descLower.includes(keyword)) {
      const requiresAuth = hint.requiresAuth || /\bmy\b|\bmine\b/i.test(description);
      return {
        domain: hint.domain,
        url: hint.url,
        task: description,
        requiresAuth,
      };
    }
  }

  // Fall back to LLM
  const result = await llm.generateJson<ParsedIntent>(
    INTENT_SYSTEM_PROMPT,
    description,
  );

  return result;
}
