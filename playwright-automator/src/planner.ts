/**
 * Planner — Two-pass LLM strategy: plan first, then generate code.
 *
 * Calls Gemini with a focused planning prompt (no code generation) to produce
 * a structured execution plan. This plan is then injected into the code
 * generation prompt so the LLM follows a clear strategy.
 */

import { GoogleGenerativeAI } from '@google/generative-ai';
import type { ExecutionPlan } from './types.js';
import { loadPrompt } from './prompts.js';

/**
 * Generate an execution plan using Gemini.
 *
 * @returns The execution plan, or null if planning fails (LLM returns bad JSON).
 */
export async function generatePlan(
  taskDescription: string,
  irSummary: string,
  workflowSummary: string,
  timelineSummary: string,
  apiKey: string,
): Promise<ExecutionPlan | null> {
  const genAI = new GoogleGenerativeAI(apiKey);
  const model = genAI.getGenerativeModel({
    model: 'gemini-3-pro-preview',
    generationConfig: {
      temperature: 0.1,
      maxOutputTokens: 4000,
    },
  });

  const promptPreamble = loadPrompt('plan');

  const prompt = `${promptPreamble}

## Task
${taskDescription}

## Endpoint Catalog (IR)
${irSummary}

${workflowSummary}

## Correlated Action-API Timeline
${timelineSummary}

Now produce the JSON execution plan:`;

  console.log('🧠 Generating execution plan...');

  try {
    const result = await model.generateContent(prompt);
    let text = result.response.text().trim();

    // Strip markdown fences if present
    text = text.replace(/^```(?:json)?\n?/gm, '');
    text = text.replace(/\n?```$/gm, '');
    text = text.trim();

    const plan = JSON.parse(text) as ExecutionPlan;

    // Basic validation
    if (!plan.taskSummary || !Array.isArray(plan.steps) || plan.steps.length === 0) {
      console.log('⚠️  Plan validation failed — skipping planning phase');
      return null;
    }

    console.log(`✅ Execution plan: ${plan.steps.length} steps`);
    for (const step of plan.steps) {
      console.log(`   ${step.step}. ${step.description} (${step.endpoint})`);
    }

    return plan;
  } catch (err) {
    console.log(`⚠️  Planning phase failed (${err instanceof Error ? err.message : 'unknown error'}) — proceeding without plan`);
    return null;
  }
}

/**
 * Render an execution plan as a string for injection into the generation prompt.
 */
export function renderPlanForPrompt(plan: ExecutionPlan): string {
  const lines: string[] = [];
  lines.push(`## Execution Plan`);
  lines.push(`Task: ${plan.taskSummary}`);
  lines.push('');
  lines.push('Follow this plan when writing the script:');

  for (const step of plan.steps) {
    let desc = `${step.step}. **${step.description}** — ${step.endpoint} (purpose: ${step.purpose})`;
    if (step.inputFrom) {
      desc += ` — uses output from step ${step.inputFrom}`;
    }
    if (step.loopOver) {
      desc += ` — loop over: \`${step.loopOver}\``;
    }
    lines.push(desc);
    if (step.triggerAction) {
      lines.push(`   — trigger: ${step.triggerAction}`);
    }
  }

  lines.push('');
  lines.push('**IMPORTANT**: Your generated script MUST implement all steps in this plan. If the plan includes a loop/iteration step, your script MUST iterate over each item.');

  return lines.join('\n');
}
