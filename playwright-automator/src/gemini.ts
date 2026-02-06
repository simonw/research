/**
 * Gemini LLM Integration — Generate Playwright automation scripts.
 *
 * Uses the recorded session data (HAR, actions, auth) to prompt Gemini
 * to generate a standalone Playwright script that automates the task.
 * Prioritizes API interception over DOM scraping.
 */

import { GoogleGenerativeAI } from '@google/generative-ai';
import crypto from 'node:crypto';
import type { RecordingSession, GenerationResult } from './types.js';
import { prepareHarSummaryForLLM } from './har-analyzer.js';
import { buildIr, summarizeIrForLLM } from './ir-builder.js';
import { buildRequestTemplates } from './request-templates.js';
import { loadPrompt, PROMPT_VERSION } from './prompts.js';
import { correlateActionsWithApis, renderTimeline } from './action-api-correlator.js';
import { analyzeWorkflow } from './workflow-analyzer.js';
import { generatePlan, renderPlanForPrompt } from './planner.js';

/**
 * Generate a Playwright automation script using Gemini.
 */
export async function generateScript(
  session: RecordingSession,
  apiKey: string,
): Promise<GenerationResult> {
  const genAI = new GoogleGenerativeAI(apiKey);
  const model = genAI.getGenerativeModel({
    model: 'gemini-3-pro-preview',
    generationConfig: {
      temperature: 0.2,
      maxOutputTokens: 16000,
    },
  });

  // Prepare IR + HAR summary for the LLM
  const ir = buildIr(session.url, session.apiRequests, {
    authMethod: session.authMethod,
    cookies: session.cookies,
    authHeaders: session.authHeaders,
  });
  const irSummary = summarizeIrForLLM(ir, 25);
  const harSummary = prepareHarSummaryForLLM(session.apiRequests, 40);
  const requestTemplates = buildRequestTemplates(session.apiRequests, 20);

  const promptPreamble = loadPrompt('generate');
  const irHash = crypto.createHash('sha256').update(JSON.stringify(ir)).digest('hex');
  const templatesHash = crypto.createHash('sha256').update(JSON.stringify(requestTemplates)).digest('hex');

  // Prepare correlated action-API timeline (or fall back to flat list)
  const timeline = session.timeline ?? correlateActionsWithApis(session.actions, session.apiRequests);
  const timelineSummary = renderTimeline(timeline);

  // Prepare workflow analysis (list→detail, pagination, variable flow)
  const workflow = session.workflowAnalysis ?? analyzeWorkflow(session.apiRequests);
  const workflowSummary = workflow.summary;

  // Also prepare flat actions summary as fallback
  const actionsSummary = session.actions
    .map((a, i) => {
      switch (a.type) {
        case 'navigate':
          return `${i + 1}. Navigate to: ${a.url}`;
        case 'click':
          return `${i + 1}. Click: ${a.selector} (${a.tagName}: "${a.text?.slice(0, 50)}")`;
        case 'type':
          return `${i + 1}. Type "${a.value}" into: ${a.selector}`;
        case 'press':
          return `${i + 1}. Press ${a.key} on: ${a.selector}`;
        default:
          return `${i + 1}. ${a.type}: ${a.description || a.selector || ''}`;
      }
    })
    .join('\n');

  // Prepare auth summary
  const authSummary = [
    `Auth method: ${session.authMethod}`,
    `Auth headers: ${JSON.stringify(session.authHeaders, null, 2)}`,
    `Number of cookies: ${Object.keys(session.cookies).length}`,
    `Key cookies: ${Object.keys(session.cookies).slice(0, 10).join(', ')}`,
    `Note: auth.json contains "playwrightCookies" (Playwright Cookie[] array for addCookies()) and "cookies" (flat Record<string,string> — do NOT pass to addCookies).`,
  ].join('\n');

  // Two-pass planning: generate a strategy plan first, then inject into code generation
  const executionPlan = await generatePlan(
    session.description,
    irSummary,
    workflowSummary,
    timelineSummary,
    apiKey,
  );
  const planSection = executionPlan ? '\n' + renderPlanForPrompt(executionPlan) + '\n' : '';

  const prompt = `${promptPreamble}

You are an expert Playwright automation engineer. Your task is to generate a standalone Playwright script that automates a browser task.

## Task Description
The user wants to: **${session.description}**
Target URL: ${session.url}
Target domain: ${session.targetDomain}

## Recorded User Actions (with correlated API calls)
**The timeline below shows exactly which user actions triggered which API calls. Your script should replicate these actions (navigate, click) to trigger the same APIs, then intercept them with \`waitForResponse\`.**
${timelineSummary || actionsSummary || 'No actions recorded (user may have only browsed)'}

## Derived Endpoint Catalog (IR)
This is a deterministic, scored summary of endpoints extracted from the HAR (use this to pick the best APIs):
${irSummary}

${workflowSummary}

## Captured API Traffic (from HAR recording)
These API calls were made during the session:
${harSummary || 'No API requests captured'}

## Authentication Info
${authSummary}

## Request Templates (for reference)
These templates show the shape of API requests observed during recording. Use them to understand endpoints, not to call directly.
IMPORTANT: these templates omit sensitive headers (Authorization/Cookie). Use storageState/cookies for auth.
Templates:
${JSON.stringify(requestTemplates.slice(0, 8), null, 2)}

**UI-driven detail collection pattern** — replicate user actions to trigger APIs, then intercept responses:
\`\`\`
// Step 1: Intercept the list API by navigating (same as user did during recording)
const listPromise = page.waitForResponse(
  r => r.url().includes('/api/items') && r.status() === 200,
  { timeout: 15000 }
);
await page.goto('https://example.com/items');
const listData = await (await listPromise).json();

// Step 2: Click each item to trigger the detail API (same as user did during recording)
const results: any[] = [];
for (const item of items) {
  const detailPromise = page.waitForResponse(
    r => r.url().includes('/api/items/') && r.status() === 200,
    { timeout: 15000 }
  );
  await page.click(\`[data-item-id="\${item.id}"]\`); // or text selector
  const detail = await (await detailPromise).json();
  results.push(detail);
  await page.goBack();
  await page.waitForLoadState('networkidle');
}
\`\`\`
**Why UI-driven?** Many sites reject bare \`fetch()\` calls — CSRF tokens, sentinel headers, proof-of-work challenges all get stripped. Click the UI to trigger APIs through the browser's natural request pipeline.

${planSection}## Instructions

Generate a complete, standalone Playwright script (TypeScript) that accomplishes the task described above.

**CRITICAL PRIORITIES (in order):**

1. **API Interception First**: Whenever possible, use \`page.waitForResponse()\` and/or \`page.on('response')\` to capture API responses rather than scraping the DOM. API data is more reliable and structured.

   **WARNING — waitForResponse predicate**: ALWAYS use predicate form, NEVER pass a full URL string:
   \`\`\`
   // CORRECT — matches by path substring
   const resp = await page.waitForResponse(r => r.url().includes('/api/conversations'));
   // WRONG — exact match will fail if query params differ
   const resp = await page.waitForResponse('https://example.com/api/conversations?limit=20');
   \`\`\`
   Query parameters, pagination limits, and tokens change between sessions — exact string match WILL timeout.

   **IMPORTANT**: Do NOT use \`route.continue()\` as if it returns a response (it returns void). If you need to actively re-issue requests, use either:
   - \`page.request.fetch(...)\` (Playwright APIRequestContext), or
   - \`page.evaluate(() => fetch(...))\` so cookies/CSRF apply in-page.

2. **Identify the Best API Endpoints**: Look at the captured API traffic and identify which endpoints return the data the user needs. For example:
   - If the user wants DMs, look for endpoints returning message/thread data
   - If the user wants posts, look for feed/timeline endpoints
   - If the user wants contacts, look for user/contact list endpoints

3. **Use Cookies/Storage for Auth**: **NEVER use \`launchPersistentContext()\`** — it is incompatible with \`storageState\` and \`addCookies()\`. Always start with \`chromium.launch()\` + \`browser.newContext()\`.

   **Auth loading code (use this exact pattern):**
   \`\`\`
   import { chromium } from 'playwright';
   import { existsSync, readFileSync } from 'fs';

   const browser = await chromium.launch({ headless: false });
   let context;
   if (existsSync('./storageState.json')) {
     context = await browser.newContext({ storageState: './storageState.json' });
   } else {
     context = await browser.newContext();
     const authData = JSON.parse(readFileSync('./auth.json', 'utf-8'));
     if (authData.playwrightCookies) await context.addCookies(authData.playwrightCookies);
   }
   const page = await context.newPage();
   \`\`\`
   **NEVER** pass \`authData.cookies\` to \`addCookies()\` — it is a flat \`Record<string,string>\`, not a \`Cookie[]\` array.

4. **Navigate to Trigger APIs**: Navigate to the right pages to trigger the API calls, then intercept the responses.

5. **Fallback to DOM scraping**: Only if no suitable API endpoint exists, use DOM scraping as a last resort.

6. **Output the data**: The script should collect all the data and output it as JSON to a file called \`output.json\` in the same directory.

7. **Pagination**: If the API supports pagination, implement pagination to get all data.

8. **Verbose Logging**: Log every major step with \`console.log\` (e.g., "Navigating to...", "Waiting for API response...", "Got N items", "Writing output..."). Add a diagnostic response logger for the target domain at the top of the script:
   \`\`\`
   page.on('response', r => {
     if (r.url().includes('${session.targetDomain}')) {
       console.log(\`  [resp] \${r.status()} \${r.url().slice(0, 120)}\`);
     }
   });
   \`\`\`
   This helps debug which API calls fire and what status codes they return.

## Output Format

Generate ONLY the TypeScript code. The script should:
- Import from 'playwright' (using \`import { chromium } from 'playwright'\`)
- Be completely self-contained and runnable with \`npx tsx script.ts\`
- Load auth from \`auth.json\` in the same directory
- Output data to \`output.json\` in the same directory
- Include clear comments explaining the strategy
- Handle errors gracefully
- Include a \`main()\` async function that runs on import

Start with a comment block explaining:
1. What strategy is being used (API interception vs DOM scraping)
2. Which specific API endpoints are being targeted
3. How auth is handled

Then the full script code.

Do NOT include markdown code fences - output only raw TypeScript.`;

  console.log('🤖 Sending recording data to Gemini for script generation...');

  const result = await model.generateContent(prompt);
  const response = result.response;
  let text = response.text();

  // Clean up response - remove markdown code fences if present
  text = text.replace(/^```(?:typescript|ts|javascript|js)?\n?/gm, '');
  text = text.replace(/\n?```$/gm, '');
  text = text.trim();

  // Extract strategy and endpoint info from the generated comments
  const strategyMatch = text.match(/\/\*\*[\s\S]*?\*\//);
  const strategy = strategyMatch ? strategyMatch[0] : 'See script comments';

  // Find mentioned API endpoints
  const endpointMatches = text.match(/(?:waitForResponse|route)\s*\(\s*['"`]([^'"`]+)/g) || [];
  const apiEndpoints = endpointMatches.map(m => {
    const match = m.match(/['"`]([^'"`]+)/);
    return match ? match[1] : '';
  }).filter(Boolean);

  return {
    script: text,
    explanation: strategy,
    apiEndpoints,
    strategy: apiEndpoints.length > 0 ? 'API Interception' : 'DOM Scraping',
    promptVersion: PROMPT_VERSION,
    irHash,
    templatesHash,
    executionPlan: executionPlan ?? undefined,
  };
}

/**
 * Refine a generated script based on feedback.
 */
export async function refineScript(
  session: RecordingSession,
  previousScript: string,
  feedback: string,
  apiKey: string,
): Promise<GenerationResult> {
  const genAI = new GoogleGenerativeAI(apiKey);
  const model = genAI.getGenerativeModel({
    model: 'gemini-3-pro-preview',
    generationConfig: {
      temperature: 0.2,
      maxOutputTokens: 16000,
    },
  });

  const ir = buildIr(session.url, session.apiRequests, {
    authMethod: session.authMethod,
    cookies: session.cookies,
    authHeaders: session.authHeaders,
  });
  const irSummary = summarizeIrForLLM(ir, 25);
  const harSummary = prepareHarSummaryForLLM(session.apiRequests, 40);
  const requestTemplates = buildRequestTemplates(session.apiRequests, 20);

  const promptPreamble = loadPrompt('refine');
  const irHash = crypto.createHash('sha256').update(JSON.stringify(ir)).digest('hex');
  const templatesHash = crypto.createHash('sha256').update(JSON.stringify(requestTemplates)).digest('hex');

  // Prepare timeline and workflow for refine context
  const refineTimeline = session.timeline ?? correlateActionsWithApis(session.actions, session.apiRequests);
  const refineTimelineSummary = renderTimeline(refineTimeline);
  const refineWorkflow = session.workflowAnalysis ?? analyzeWorkflow(session.apiRequests);
  const refinePlanSection = session.executionPlan ? '\n' + renderPlanForPrompt(session.executionPlan) + '\n' : '';

  const prompt = `${promptPreamble}

You are an expert Playwright automation engineer. You need to fix/improve a previously generated script.

## Original Task
The user wants to: **${session.description}**
Target URL: ${session.url}

## Previous Script
\`\`\`typescript
${previousScript}
\`\`\`

## User Feedback
${feedback}

## Recorded User Actions (with correlated API calls)
${refineTimelineSummary}

## Derived Endpoint Catalog (IR)
${irSummary}

${refineWorkflow.summary}

## Available API Traffic (for reference)
${harSummary}

## Request Templates (for API replay fallback)
${JSON.stringify(requestTemplates.slice(0, 8), null, 2)}

## Auth Info
Auth method: ${session.authMethod}
Target domain: ${session.targetDomain}
Auth headers: ${JSON.stringify(session.authHeaders, null, 2)}
Number of cookies: ${Object.keys(session.cookies).length}
Key cookies: ${Object.keys(session.cookies).slice(0, 10).join(', ')}

**auth.json format:**
- \`playwrightCookies\`: Playwright \`Cookie[]\` array — use with \`context.addCookies(authData.playwrightCookies)\`
- \`cookies\`: flat \`Record<string,string>\` — NEVER pass to \`addCookies()\`
- \`authHeaders\`: object with auth header name→value (e.g. \`{ "authorization": "Bearer ..." }\`)
- Prefer \`storageState.json\` when available: \`browser.newContext({ storageState: './storageState.json' })\`
${refinePlanSection}
## Instructions
Fix the script based on the user's feedback. Keep the same overall approach but address the issues mentioned.

**Common bugs checklist — fix these if present:**
- \`waitForResponse('https://...')\` exact string match → use predicate form: \`waitForResponse(r => r.url().includes('/path'))\`
- \`launchPersistentContext()\` → replace with \`chromium.launch()\` + \`browser.newContext()\`
- Missing logging → add \`console.log\` at every major step + diagnostic \`page.on('response')\` logger for the target domain
- No UI-driven iteration → if the plan includes a list→detail loop, click each item in the UI and intercept the detail API via \`waitForResponse\`, don't use \`page.evaluate(() => fetch(...))\` to iterate
- \`page.evaluate(() => fetch(...))\` for iteration → replace with UI clicks + \`page.waitForResponse()\` interception. Many sites reject bare fetch() (missing CSRF, sentinel headers, proof-of-work)

Output ONLY the TypeScript code (no markdown fences). The script should be complete and runnable.`;

  console.log('🤖 Refining script with Gemini...');

  const result = await model.generateContent(prompt);
  let text = result.response.text();

  text = text.replace(/^```(?:typescript|ts|javascript|js)?\n?/gm, '');
  text = text.replace(/\n?```$/gm, '');
  text = text.trim();

  const endpointMatches = text.match(/(?:waitForResponse|route)\s*\(\s*['"`]([^'"`]+)/g) || [];
  const apiEndpoints = endpointMatches.map(m => {
    const match = m.match(/['"`]([^'"`]+)/);
    return match ? match[1] : '';
  }).filter(Boolean);

  return {
    script: text,
    explanation: 'Refined based on feedback',
    apiEndpoints,
    strategy: apiEndpoints.length > 0 ? 'API Interception' : 'DOM Scraping',
    promptVersion: PROMPT_VERSION,
    irHash,
    templatesHash,
  };
}
