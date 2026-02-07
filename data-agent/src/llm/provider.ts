/**
 * Pluggable LLM provider interface.
 *
 * Supports Gemini and Claude. Provider selection via env vars or CLI flag.
 */

import type { LlmProvider } from '../types.js';

export function createLlmProvider(name: string = 'gemini'): LlmProvider {
  switch (name.toLowerCase()) {
    case 'gemini':
      return createGeminiProvider();
    case 'claude':
      return createClaudeProvider();
    default:
      throw new Error(`Unknown LLM provider: ${name}. Supported: gemini, claude`);
  }
}

function createGeminiProvider(): LlmProvider {
  const apiKey = process.env.GEMINI_API_KEY;
  if (!apiKey) {
    throw new Error('GEMINI_API_KEY environment variable is required for Gemini provider');
  }

  return {
    name: 'gemini',

    async generateText(systemPrompt: string, userPrompt: string): Promise<string> {
      const { GoogleGenerativeAI } = await import('@google/generative-ai');
      const genAI = new GoogleGenerativeAI(apiKey);
      const model = genAI.getGenerativeModel({
        model: 'gemini-2.5-pro-preview-05-06',
        generationConfig: {
          temperature: 0.2,
          maxOutputTokens: 16000,
        },
      });

      const result = await model.generateContent({
        systemInstruction: systemPrompt,
        contents: [{ role: 'user', parts: [{ text: userPrompt }] }],
      });

      return result.response.text();
    },

    async generateJson<T>(systemPrompt: string, userPrompt: string): Promise<T> {
      const text = await this.generateText(systemPrompt, userPrompt);
      // Strip markdown fences if present
      const cleaned = text.replace(/^```(?:json)?\n?/m, '').replace(/\n?```$/m, '').trim();
      return JSON.parse(cleaned) as T;
    },
  };
}

function createClaudeProvider(): LlmProvider {
  const apiKey = process.env.ANTHROPIC_API_KEY;
  if (!apiKey) {
    throw new Error('ANTHROPIC_API_KEY environment variable is required for Claude provider');
  }

  return {
    name: 'claude',

    async generateText(systemPrompt: string, userPrompt: string): Promise<string> {
      const { default: Anthropic } = await import('@anthropic-ai/sdk');
      const client = new Anthropic({ apiKey });

      const message = await client.messages.create({
        model: 'claude-sonnet-4-5-20250929',
        max_tokens: 16000,
        system: systemPrompt,
        messages: [{ role: 'user', content: userPrompt }],
      });

      const block = message.content[0];
      if (block.type !== 'text') throw new Error('Expected text response from Claude');
      return block.text;
    },

    async generateJson<T>(systemPrompt: string, userPrompt: string): Promise<T> {
      const text = await this.generateText(systemPrompt, userPrompt);
      const cleaned = text.replace(/^```(?:json)?\n?/m, '').replace(/\n?```$/m, '').trim();
      return JSON.parse(cleaned) as T;
    },
  };
}
