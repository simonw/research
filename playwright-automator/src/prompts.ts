import { readFileSync } from 'node:fs';
import { join } from 'node:path';

export const PROMPT_VERSION = 'v1' as const;

export function loadPrompt(name: 'generate'|'refine'): string {
  const p = join(process.cwd(), 'prompts', PROMPT_VERSION, `${name}.md`);
  return readFileSync(p, 'utf-8');
}
