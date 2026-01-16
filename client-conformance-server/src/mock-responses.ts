/**
 * Mock response generator for client conformance testing.
 * Generates valid OpenResponses API responses based on test scenarios.
 */

import type { CreateResponseBody, ResponseResource } from "./schemas.js";

let responseIdCounter = 0;
let itemIdCounter = 0;

function generateId(prefix: string): string {
  return `${prefix}_${Date.now()}_${++responseIdCounter}`;
}

function generateItemId(prefix: string): string {
  return `${prefix}_${++itemIdCounter}`;
}

export interface MockResponseOptions {
  model: string;
  request: CreateResponseBody;
}

/**
 * Generate a basic text response
 */
export function generateBasicResponse(options: MockResponseOptions): ResponseResource {
  const now = Math.floor(Date.now() / 1000);
  const messageId = generateItemId("msg");

  return {
    id: generateId("resp"),
    object: "response",
    created_at: now,
    completed_at: now + 1,
    status: "completed",
    model: options.model,
    output: [
      {
        type: "message",
        id: messageId,
        role: "assistant",
        status: "completed",
        content: [
          {
            type: "output_text",
            text: "Hello! This is a mock response from the conformance test server.",
          },
        ],
      },
    ],
    usage: {
      input_tokens: 10,
      output_tokens: 15,
      total_tokens: 25,
    },
  };
}

/**
 * Generate a tool call response
 */
export function generateToolCallResponse(
  options: MockResponseOptions,
  toolName: string,
  toolArgs: Record<string, unknown>
): ResponseResource {
  const now = Math.floor(Date.now() / 1000);
  const callId = generateItemId("call");

  return {
    id: generateId("resp"),
    object: "response",
    created_at: now,
    completed_at: now + 1,
    status: "completed",
    model: options.model,
    output: [
      {
        type: "function_call",
        id: generateItemId("fc"),
        call_id: callId,
        name: toolName,
        arguments: JSON.stringify(toolArgs),
        status: "completed",
      },
    ],
    tools: options.request.tools,
    usage: {
      input_tokens: 20,
      output_tokens: 10,
      total_tokens: 30,
    },
  };
}

/**
 * Generate a multi-turn context response that echoes back conversation details
 */
export function generateMultiTurnResponse(options: MockResponseOptions): ResponseResource {
  const now = Math.floor(Date.now() / 1000);
  const messageId = generateItemId("msg");

  // Count messages in the input
  const input = options.request.input;
  let messageCount = 0;
  if (Array.isArray(input)) {
    messageCount = input.filter((item) => item.type === "message").length;
  }

  return {
    id: generateId("resp"),
    object: "response",
    created_at: now,
    completed_at: now + 1,
    status: "completed",
    model: options.model,
    output: [
      {
        type: "message",
        id: messageId,
        role: "assistant",
        status: "completed",
        content: [
          {
            type: "output_text",
            text: `I received ${messageCount} messages in this conversation. The context was properly preserved.`,
          },
        ],
      },
    ],
    usage: {
      input_tokens: 50,
      output_tokens: 20,
      total_tokens: 70,
    },
  };
}

/**
 * Generate a failed response
 */
export function generateFailedResponse(
  options: MockResponseOptions,
  errorCode: string,
  errorMessage: string
): ResponseResource {
  const now = Math.floor(Date.now() / 1000);

  return {
    id: generateId("resp"),
    object: "response",
    created_at: now,
    completed_at: now + 1,
    status: "failed",
    model: options.model,
    output: [],
    error: {
      code: errorCode,
      message: errorMessage,
    },
  };
}

/**
 * Generate streaming events for SSE response
 */
export interface StreamingEvent {
  event: string;
  data: unknown;
}

export function generateStreamingEvents(options: MockResponseOptions): StreamingEvent[] {
  const now = Math.floor(Date.now() / 1000);
  const responseId = generateId("resp");
  const messageId = generateItemId("msg");
  const textParts = ["Hello", "! This", " is a", " streaming", " response", "."];

  const events: StreamingEvent[] = [];
  let sequenceNumber = 0;

  // Initial response created event
  const baseResponse: ResponseResource = {
    id: responseId,
    object: "response",
    created_at: now,
    status: "in_progress",
    model: options.model,
    output: [],
  };

  events.push({
    event: "response.created",
    data: {
      type: "response.created",
      sequence_number: sequenceNumber++,
      response: baseResponse,
    },
  });

  // In progress event
  events.push({
    event: "response.in_progress",
    data: {
      type: "response.in_progress",
      sequence_number: sequenceNumber++,
      response: { ...baseResponse, status: "in_progress" },
    },
  });

  // Output item added
  events.push({
    event: "response.output_item.added",
    data: {
      type: "response.output_item.added",
      sequence_number: sequenceNumber++,
      output_index: 0,
      item: {
        type: "message",
        id: messageId,
        role: "assistant",
        status: "in_progress",
        content: [],
      },
    },
  });

  // Content part added
  events.push({
    event: "response.content_part.added",
    data: {
      type: "response.content_part.added",
      sequence_number: sequenceNumber++,
      item_id: messageId,
      output_index: 0,
      content_index: 0,
      part: {
        type: "output_text",
        text: "",
      },
    },
  });

  // Text deltas
  let fullText = "";
  for (const part of textParts) {
    fullText += part;
    events.push({
      event: "response.output_text.delta",
      data: {
        type: "response.output_text.delta",
        sequence_number: sequenceNumber++,
        item_id: messageId,
        output_index: 0,
        content_index: 0,
        delta: part,
      },
    });
  }

  // Text done
  events.push({
    event: "response.output_text.done",
    data: {
      type: "response.output_text.done",
      sequence_number: sequenceNumber++,
      item_id: messageId,
      output_index: 0,
      content_index: 0,
      text: fullText,
    },
  });

  // Content part done
  events.push({
    event: "response.content_part.done",
    data: {
      type: "response.content_part.done",
      sequence_number: sequenceNumber++,
      item_id: messageId,
      output_index: 0,
      content_index: 0,
      part: {
        type: "output_text",
        text: fullText,
      },
    },
  });

  // Output item done
  events.push({
    event: "response.output_item.done",
    data: {
      type: "response.output_item.done",
      sequence_number: sequenceNumber++,
      output_index: 0,
      item: {
        type: "message",
        id: messageId,
        role: "assistant",
        status: "completed",
        content: [
          {
            type: "output_text",
            text: fullText,
          },
        ],
      },
    },
  });

  // Response completed
  const completedResponse: ResponseResource = {
    id: responseId,
    object: "response",
    created_at: now,
    completed_at: now + 1,
    status: "completed",
    model: options.model,
    output: [
      {
        type: "message",
        id: messageId,
        role: "assistant",
        status: "completed",
        content: [
          {
            type: "output_text",
            text: fullText,
          },
        ],
      },
    ],
    usage: {
      input_tokens: 10,
      output_tokens: textParts.length,
      total_tokens: 10 + textParts.length,
    },
  };

  events.push({
    event: "response.completed",
    data: {
      type: "response.completed",
      sequence_number: sequenceNumber++,
      response: completedResponse,
    },
  });

  return events;
}

/**
 * Generate streaming events for a tool call
 */
export function generateToolCallStreamingEvents(
  options: MockResponseOptions,
  toolName: string,
  toolArgs: Record<string, unknown>
): StreamingEvent[] {
  const now = Math.floor(Date.now() / 1000);
  const responseId = generateId("resp");
  const functionCallId = generateItemId("fc");
  const callId = generateItemId("call");
  const argsStr = JSON.stringify(toolArgs);
  const argParts = [argsStr.slice(0, argsStr.length / 2), argsStr.slice(argsStr.length / 2)];

  const events: StreamingEvent[] = [];
  let sequenceNumber = 0;

  const baseResponse: ResponseResource = {
    id: responseId,
    object: "response",
    created_at: now,
    status: "in_progress",
    model: options.model,
    output: [],
  };

  events.push({
    event: "response.created",
    data: {
      type: "response.created",
      sequence_number: sequenceNumber++,
      response: baseResponse,
    },
  });

  events.push({
    event: "response.in_progress",
    data: {
      type: "response.in_progress",
      sequence_number: sequenceNumber++,
      response: { ...baseResponse },
    },
  });

  events.push({
    event: "response.output_item.added",
    data: {
      type: "response.output_item.added",
      sequence_number: sequenceNumber++,
      output_index: 0,
      item: {
        type: "function_call",
        id: functionCallId,
        call_id: callId,
        name: toolName,
        arguments: "",
        status: "in_progress",
      },
    },
  });

  // Arguments deltas
  let fullArgs = "";
  for (const part of argParts) {
    fullArgs += part;
    events.push({
      event: "response.function_call_arguments.delta",
      data: {
        type: "response.function_call_arguments.delta",
        sequence_number: sequenceNumber++,
        item_id: functionCallId,
        output_index: 0,
        call_id: callId,
        delta: part,
      },
    });
  }

  events.push({
    event: "response.function_call_arguments.done",
    data: {
      type: "response.function_call_arguments.done",
      sequence_number: sequenceNumber++,
      item_id: functionCallId,
      output_index: 0,
      call_id: callId,
      arguments: fullArgs,
    },
  });

  events.push({
    event: "response.output_item.done",
    data: {
      type: "response.output_item.done",
      sequence_number: sequenceNumber++,
      output_index: 0,
      item: {
        type: "function_call",
        id: functionCallId,
        call_id: callId,
        name: toolName,
        arguments: fullArgs,
        status: "completed",
      },
    },
  });

  const completedResponse: ResponseResource = {
    id: responseId,
    object: "response",
    created_at: now,
    completed_at: now + 1,
    status: "completed",
    model: options.model,
    output: [
      {
        type: "function_call",
        id: functionCallId,
        call_id: callId,
        name: toolName,
        arguments: fullArgs,
        status: "completed",
      },
    ],
    tools: options.request.tools,
    usage: {
      input_tokens: 20,
      output_tokens: 10,
      total_tokens: 30,
    },
  };

  events.push({
    event: "response.completed",
    data: {
      type: "response.completed",
      sequence_number: sequenceNumber++,
      response: completedResponse,
    },
  });

  return events;
}
