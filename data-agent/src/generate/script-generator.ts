/**
 * Script Generator — LLM generates standalone Playwright automation scripts from analysis.
 *
 * Takes the analysis result (IR, workflow, timeline, templates) and produces
 * a pure Playwright TypeScript script that extracts the requested data.
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import type { AnalysisResult, GenerationResult, LlmProvider } from '../types.js';
import { summarizeIrForLLM } from '../analyze/ir-builder.js';
import { renderTimeline } from '../analyze/correlator.js';

interface GenerateOptions {
  analysis: AnalysisResult;
  task: string;
  url: string;
  llm: LlmProvider;
  sessionDir: string;
}

/**
 * Load the generate system prompt.
 */
function loadGeneratePrompt(): string {
  try {
    const promptPath = resolve(import.meta.dirname, '../../prompts/generate.md');
    return readFileSync(promptPath, 'utf-8');
  } catch {
    return 'You are an expert Playwright automation engineer. Generate a standalone TypeScript script.';
  }
}

/**
 * Generate an automation script from the analysis.
 */
export async function generateScript(options: GenerateOptions): Promise<GenerationResult> {
  const { analysis, task, url, llm, sessionDir } = options;

  const systemPrompt = loadGeneratePrompt();

  // Build the user prompt with all available context
  const userPrompt = buildGeneratePrompt(task, url, analysis);

  // Generate script via LLM
  let script = await llm.generateText(systemPrompt, userPrompt);

  // Clean up markdown fences if present
  script = script.replace(/^```(?:typescript|ts)?\n?/m, '').replace(/\n?```$/m, '').trim();

  // Write script to session directory
  writeFileSync(join(sessionDir, 'automation.ts'), script, 'utf-8');

  const result: GenerationResult = {
    script,
    explanation: 'Generated from analysis',
    apiEndpoints: analysis.ir.endpoints.slice(0, 5).map(ep => `${ep.method} ${ep.pathPattern}`),
    strategy: analysis.workflow.patterns.length > 0
      ? analysis.workflow.patterns[0].type
      : 'direct-api',
  };

  return result;
}

/**
 * Build the comprehensive user prompt for script generation.
 */
function buildGeneratePrompt(task: string, url: string, analysis: AnalysisResult): string {
  const parts: string[] = [];

  parts.push(`## Task\n${task}\n`);
  parts.push(`## Target URL\n${url}\n`);

  // IR summary
  parts.push('## API Endpoint Catalog (scored)\n');
  parts.push(summarizeIrForLLM(analysis.ir));
  parts.push('');

  // Workflow patterns
  if (analysis.workflow.patterns.length > 0) {
    parts.push(analysis.workflow.summary);
    parts.push('');
  }

  // Correlated timeline
  const timelineStr = renderTimeline(analysis.timeline);
  if (timelineStr) {
    parts.push('## Correlated Timeline (user actions → API calls)\n');
    parts.push(timelineStr);
    parts.push('');
  }

  // Request templates (top 10)
  if (analysis.requestTemplates.length > 0) {
    parts.push('## Request Templates (sanitized)');
    for (const tmpl of analysis.requestTemplates.slice(0, 10)) {
      parts.push(`\n### ${tmpl.method} ${tmpl.url.slice(0, 120)}`);
      if (tmpl.body) parts.push(`Body: ${tmpl.body.slice(0, 500)}`);
    }
    parts.push('');
  }

  parts.push('Generate the automation.ts script now. Output ONLY raw TypeScript code.');

  return parts.join('\n');
}

/**
 * Refine a previously generated script based on execution feedback.
 */
export async function refineScript(options: {
  sessionDir: string;
  previousScript: string;
  feedback: string;
  analysis: AnalysisResult;
  task: string;
  url: string;
  llm: LlmProvider;
}): Promise<GenerationResult> {
  const { sessionDir, previousScript, feedback, analysis, task, url, llm } = options;

  let diagnosePrompt: string;
  try {
    const promptPath = resolve(import.meta.dirname, '../../prompts/diagnose.md');
    diagnosePrompt = readFileSync(promptPath, 'utf-8');
  } catch {
    diagnosePrompt = 'You are an expert Playwright engineer. Refine the script based on feedback.';
  }

  const userPrompt = [
    `## Task\n${task}\n`,
    `## Target URL\n${url}\n`,
    `## Previous Script\n\`\`\`typescript\n${previousScript}\n\`\`\`\n`,
    `## Execution Feedback\n${feedback}\n`,
    `## API Endpoint Catalog\n${summarizeIrForLLM(analysis.ir)}\n`,
    'Generate the refined automation.ts script now. Output ONLY raw TypeScript code.',
  ].join('\n');

  let script = await llm.generateText(diagnosePrompt, userPrompt);
  script = script.replace(/^```(?:typescript|ts)?\n?/m, '').replace(/\n?```$/m, '').trim();

  writeFileSync(join(sessionDir, 'automation.ts'), script, 'utf-8');

  return {
    script,
    explanation: 'Refined from feedback',
    apiEndpoints: analysis.ir.endpoints.slice(0, 5).map(ep => `${ep.method} ${ep.pathPattern}`),
    strategy: 'refined',
  };
}
