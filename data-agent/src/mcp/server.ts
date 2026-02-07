/**
 * MCP Server — Expose data-agent as an MCP tool server.
 *
 * Tools:
 *   - explore: Explore a site and extract data
 *   - replay: Replay a saved extractor
 *   - list: List available extractors
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from '@modelcontextprotocol/sdk/types.js';
import { resolve } from 'node:path';
import { mkdirSync, existsSync, readFileSync } from 'node:fs';
import { explore } from '../explore/explorer.js';
import { analyze } from '../analyze/har-analyzer.js';
import { generateScript } from '../generate/script-generator.js';
import { validate } from '../validate/validator.js';
import { findExtractor, listRegistry, publishToRegistry } from '../replay/registry.js';
import { replay } from '../replay/replay.js';
import { parseIntent } from '../explore/intent-parser.js';
import { createLlmProvider } from '../llm/provider.js';

const SESSIONS_DIR = resolve(process.cwd(), 'sessions');

export async function startMcpServer(): Promise<void> {
  const server = new Server(
    { name: 'data-agent', version: '0.1.0' },
    { capabilities: { tools: {} } },
  );

  server.setRequestHandler(ListToolsRequestSchema, async () => ({
    tools: [
      {
        name: 'explore',
        description: 'Explore a website to discover and extract data. Uses AI to navigate the site, discover API endpoints, generate a Playwright automation script, and return the extracted data.',
        inputSchema: {
          type: 'object' as const,
          properties: {
            prompt: {
              type: 'string',
              description: 'Natural language description of what data to extract (e.g. "get the top 10 stories from hacker news")',
            },
            url: {
              type: 'string',
              description: 'Optional starting URL. If not provided, will be inferred from the prompt.',
            },
            provider: {
              type: 'string',
              description: 'LLM provider to use (gemini or claude). Default: gemini',
              enum: ['gemini', 'claude'],
            },
          },
          required: ['prompt'],
        },
      },
      {
        name: 'replay',
        description: 'Replay a previously saved extractor for a domain. No LLM required — runs the saved Playwright script deterministically.',
        inputSchema: {
          type: 'object' as const,
          properties: {
            domain: {
              type: 'string',
              description: 'Domain to replay (e.g. "news.ycombinator.com")',
            },
          },
          required: ['domain'],
        },
      },
      {
        name: 'list',
        description: 'List all available extractors in the registry.',
        inputSchema: {
          type: 'object' as const,
          properties: {},
        },
      },
    ],
  }));

  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const { name, arguments: args } = request.params;

    switch (name) {
      case 'explore': {
        const prompt = (args as Record<string, unknown>).prompt as string;
        const urlArg = (args as Record<string, unknown>).url as string | undefined;
        const provider = ((args as Record<string, unknown>).provider as string) ?? 'gemini';

        if (!existsSync(SESSIONS_DIR)) mkdirSync(SESSIONS_DIR, { recursive: true });

        const llm = createLlmProvider(provider);
        const intent = await parseIntent(prompt, llm);
        const url = urlArg ?? intent.url;

        const exploreResult = await explore({
          task: intent.task, url, llm,
          headless: true,
          sessionsDir: SESSIONS_DIR,
        });

        const analysis = await analyze(exploreResult);

        await generateScript({
          analysis, task: intent.task, url, llm,
          sessionDir: exploreResult.sessionDir,
        });

        const validationResult = await validate({
          sessionDir: exploreResult.sessionDir,
          llm, maxIterations: 3,
        });

        if (validationResult.success) {
          await publishToRegistry({
            domain: intent.domain,
            task: intent.task,
            sessionDir: exploreResult.sessionDir,
          });

          const outputPath = `${exploreResult.sessionDir}/output.json`;
          let output = '';
          try { output = readFileSync(outputPath, 'utf-8'); } catch { /* ok */ }

          return {
            content: [{
              type: 'text',
              text: `Successfully extracted data from ${intent.domain}.\n\nItems: ${validationResult.outputItemCount}\n\nData:\n${output.slice(0, 10000)}`,
            }],
          };
        }

        return {
          content: [{
            type: 'text',
            text: `Failed to extract data from ${intent.domain} after ${validationResult.iterations} iterations.`,
          }],
          isError: true,
        };
      }

      case 'replay': {
        const domain = (args as Record<string, unknown>).domain as string;
        const entry = findExtractor(domain);

        if (!entry) {
          return {
            content: [{ type: 'text', text: `No extractor found for "${domain}".` }],
            isError: true,
          };
        }

        // Run replay and capture output
        await replay(domain);

        const outputPath = `${entry.scriptPath.replace('automation.ts', 'output.json')}`;
        let output = '';
        try { output = readFileSync(outputPath, 'utf-8'); } catch { /* ok */ }

        return {
          content: [{
            type: 'text',
            text: `Replayed extractor for ${domain}.\n\nData:\n${output.slice(0, 10000)}`,
          }],
        };
      }

      case 'list': {
        const { existsSync: exists, readFileSync: readFile } = await import('node:fs');
        const { join } = await import('node:path');
        const { homedir } = await import('node:os');

        const registryPath = join(homedir(), '.data-agent', 'registry.json');
        if (!exists(registryPath)) {
          return { content: [{ type: 'text', text: 'No extractors registered yet.' }] };
        }

        const registry = JSON.parse(readFile(registryPath, 'utf-8'));
        const entries = registry.entries || [];

        if (entries.length === 0) {
          return { content: [{ type: 'text', text: 'No extractors registered yet.' }] };
        }

        const lines = entries.map((e: Record<string, unknown>) =>
          `- ${e.domain}: ${e.task} (runs: ${e.runCount})`,
        );

        return {
          content: [{
            type: 'text',
            text: `Registered extractors:\n\n${lines.join('\n')}`,
          }],
        };
      }

      default:
        return {
          content: [{ type: 'text', text: `Unknown tool: ${name}` }],
          isError: true,
        };
    }
  });

  const transport = new StdioServerTransport();
  await server.connect(transport);
}
