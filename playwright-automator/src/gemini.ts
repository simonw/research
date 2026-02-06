/**
 * Gemini LLM Integration — Generate Playwright automation scripts.
 *
 * Uses the recorded session data (HAR, actions, auth) to prompt Gemini
 * to generate a standalone Playwright script that automates the task.
 * Prioritizes API interception over DOM scraping.
 */

import { GoogleGenerativeAI } from '@google/generative-ai';
import type { RecordingSession, GenerationResult } from './types.js';
import { prepareHarSummaryForLLM } from './har-analyzer.js';

/**
 * Generate a Playwright automation script using Gemini.
 */
export async function generateScript(
  session: RecordingSession,
  apiKey: string,
): Promise<GenerationResult> {
  const genAI = new GoogleGenerativeAI(apiKey);
  const model = genAI.getGenerativeModel({
    model: 'gemini-2.0-flash',
    generationConfig: {
      temperature: 0.2,
      maxOutputTokens: 16000,
    },
  });

  // Prepare HAR summary for the LLM
  const harSummary = prepareHarSummaryForLLM(session.apiRequests, 40);

  // Prepare actions summary
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
  ].join('\n');

  const prompt = `You are an expert Playwright automation engineer. Your task is to generate a standalone Playwright script that automates a browser task.

## Task Description
The user wants to: **${session.description}**
Target URL: ${session.url}
Target domain: ${session.targetDomain}

## Recorded User Actions
The user performed these actions in the browser:
${actionsSummary || 'No actions recorded (user may have only browsed)'}

## Captured API Traffic (from HAR recording)
These API calls were made during the session:
${harSummary || 'No API requests captured'}

## Authentication Info
${authSummary}

## Instructions

Generate a complete, standalone Playwright script (TypeScript) that accomplishes the task described above.

**CRITICAL PRIORITIES (in order):**

1. **API Interception First**: Whenever possible, use \`page.route()\` or \`page.waitForResponse()\` to intercept API responses rather than scraping the DOM. API data is more reliable and structured.

2. **Identify the Best API Endpoints**: Look at the captured API traffic and identify which endpoints return the data the user needs. For example:
   - If the user wants DMs, look for endpoints returning message/thread data
   - If the user wants posts, look for feed/timeline endpoints
   - If the user wants contacts, look for user/contact list endpoints

3. **Use Cookies for Auth**: The script should load saved cookies from an \`auth.json\` file to maintain the logged-in session. Include a step to inject cookies before navigating.

4. **Navigate to Trigger APIs**: Navigate to the right pages to trigger the API calls, then intercept the responses.

5. **Fallback to DOM scraping**: Only if no suitable API endpoint exists, use DOM scraping as a last resort.

6. **Output the data**: The script should collect all the data and output it as JSON to a file called \`output.json\` in the same directory.

7. **Pagination**: If the API supports pagination, implement pagination to get all data.

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
    model: 'gemini-2.0-flash',
    generationConfig: {
      temperature: 0.2,
      maxOutputTokens: 16000,
    },
  });

  const harSummary = prepareHarSummaryForLLM(session.apiRequests, 40);

  const prompt = `You are an expert Playwright automation engineer. You need to fix/improve a previously generated script.

## Original Task
The user wants to: **${session.description}**
Target URL: ${session.url}

## Previous Script
\`\`\`typescript
${previousScript}
\`\`\`

## User Feedback
${feedback}

## Available API Traffic (for reference)
${harSummary}

## Auth Info
Auth method: ${session.authMethod}
Cookies available: ${Object.keys(session.cookies).length}

## Instructions
Fix the script based on the user's feedback. Keep the same overall approach but address the issues mentioned.

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
  };
}
