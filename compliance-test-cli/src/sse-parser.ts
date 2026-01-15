/**
 * Server-Sent Events (SSE) parser for OpenResponses API streaming responses.
 * Based on the sse-parser from the openresponses repository.
 */

import type { z } from "zod";
import { streamingEventSchema, type ResponseResource } from "./schemas.js";

export interface ParsedEvent {
  event: string;
  data: unknown;
  validationResult: z.SafeParseReturnType<unknown, z.infer<typeof streamingEventSchema>>;
}

export interface SSEParseResult {
  events: ParsedEvent[];
  errors: string[];
  finalResponse: ResponseResource | null;
}

/**
 * Parse an SSE stream from an OpenResponses API response.
 */
export async function parseSSEStream(response: Response): Promise<SSEParseResult> {
  const events: ParsedEvent[] = [];
  const errors: string[] = [];
  let finalResponse: ResponseResource | null = null;

  const reader = response.body?.getReader();
  if (!reader) {
    return { events, errors: ["No response body"], finalResponse };
  }

  const decoder = new TextDecoder();
  let buffer = "";

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() || "";

      let currentEvent = "";
      let currentData = "";

      for (const line of lines) {
        if (line.startsWith("event:")) {
          currentEvent = line.slice(6).trim();
        } else if (line.startsWith("data:")) {
          currentData = line.slice(5).trim();
        } else if (line === "" && currentData) {
          if (currentData === "[DONE]") {
            // Skip the [DONE] sentinel - it's not a real event
          } else {
            try {
              const parsed = JSON.parse(currentData);
              const validationResult = streamingEventSchema.safeParse(parsed);

              events.push({
                event: currentEvent || parsed.type || "unknown",
                data: parsed,
                validationResult,
              });

              if (!validationResult.success) {
                errors.push(
                  `Event validation failed for ${parsed.type || "unknown"}: ${JSON.stringify(validationResult.error.issues)}`
                );
              }

              if (
                parsed.type === "response.completed" ||
                parsed.type === "response.failed"
              ) {
                finalResponse = parsed.response;
              }
            } catch {
              errors.push(`Failed to parse event data: ${currentData}`);
            }
          }
          currentEvent = "";
          currentData = "";
        }
      }
    }
  } finally {
    reader.releaseLock();
  }

  return { events, errors, finalResponse };
}
