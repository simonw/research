/**
 * OpenResponses Client Conformance Test Server
 *
 * A mock server that validates client requests and returns test responses.
 */

import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { createResponseBodySchema, type CreateResponseBody } from "./schemas.js";
import {
  generateBasicResponse,
  generateToolCallResponse,
  generateMultiTurnResponse,
  generateFailedResponse,
  generateStreamingEvents,
  generateToolCallStreamingEvents,
  type StreamingEvent,
} from "./mock-responses.js";
import type { ZodIssue } from "zod";

// ============================================================================
// Types
// ============================================================================

export interface ServerConfig {
  port: number;
  host: string;
  verbose: boolean;
}

export interface ValidationReport {
  valid: boolean;
  errors: ZodIssue[];
  request?: CreateResponseBody;
  timestamp: number;
}

export interface TestScenario {
  id: string;
  name: string;
  description: string;
  trigger: string;
}

// ============================================================================
// Test Scenarios
// ============================================================================

export const testScenarios: TestScenario[] = [
  {
    id: "basic",
    name: "Basic Response",
    description: "Returns a simple text response",
    trigger: "Default behavior or model contains 'basic'",
  },
  {
    id: "streaming",
    name: "Streaming Response",
    description: "Returns SSE streaming events (set stream: true)",
    trigger: "Set stream: true in request",
  },
  {
    id: "tool-call",
    name: "Tool Call Response",
    description: "Returns a function_call output item",
    trigger: "Include tools array in request",
  },
  {
    id: "tool-call-streaming",
    name: "Tool Call Streaming",
    description: "Returns streaming tool call events",
    trigger: "Include tools array and stream: true",
  },
  {
    id: "multi-turn",
    name: "Multi-turn Context",
    description: "Echoes back conversation message count",
    trigger: "Include multiple messages in input array",
  },
  {
    id: "error-400",
    name: "Bad Request Error",
    description: "Returns HTTP 400 with error details",
    trigger: "Model contains 'error-400' or input contains 'TRIGGER:error-400'",
  },
  {
    id: "error-401",
    name: "Unauthorized Error",
    description: "Returns HTTP 401 unauthorized",
    trigger: "Model contains 'error-401' or input contains 'TRIGGER:error-401'",
  },
  {
    id: "error-429",
    name: "Rate Limit Error",
    description: "Returns HTTP 429 rate limited",
    trigger: "Model contains 'error-429' or input contains 'TRIGGER:error-429'",
  },
  {
    id: "error-500",
    name: "Internal Server Error",
    description: "Returns HTTP 500 server error",
    trigger: "Model contains 'error-500' or input contains 'TRIGGER:error-500'",
  },
  {
    id: "failed-response",
    name: "Failed Response Status",
    description: "Returns response with status: failed",
    trigger: "Model contains 'failed' or input contains 'TRIGGER:failed'",
  },
];

// ============================================================================
// Server State
// ============================================================================

let lastValidationReport: ValidationReport | null = null;
let requestCount = 0;

// ============================================================================
// Helpers
// ============================================================================

function getRequestBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => resolve(body));
    req.on("error", reject);
  });
}

function sendJson(res: ServerResponse, status: number, data: unknown): void {
  res.writeHead(status, {
    "Content-Type": "application/json",
    "Access-Control-Allow-Origin": "*",
  });
  res.end(JSON.stringify(data));
}

function sendSSE(res: ServerResponse, events: StreamingEvent[], delayMs = 50): void {
  res.writeHead(200, {
    "Content-Type": "text/event-stream",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Access-Control-Allow-Origin": "*",
  });

  let index = 0;

  function sendNextEvent(): void {
    if (index >= events.length) {
      res.write("data: [DONE]\n\n");
      res.end();
      return;
    }

    const event = events[index++];
    if (event.event) {
      res.write(`event: ${event.event}\n`);
    }
    res.write(`data: ${JSON.stringify(event.data)}\n\n`);

    setTimeout(sendNextEvent, delayMs);
  }

  sendNextEvent();
}

function detectTrigger(request: CreateResponseBody): string | null {
  const model = request.model || "";

  // Check model name for triggers
  if (model.includes("error-400")) return "error-400";
  if (model.includes("error-401")) return "error-401";
  if (model.includes("error-429")) return "error-429";
  if (model.includes("error-500")) return "error-500";
  if (model.includes("failed")) return "failed-response";

  // Check input for TRIGGER: patterns
  const input = request.input;
  if (typeof input === "string") {
    if (input.includes("TRIGGER:error-400")) return "error-400";
    if (input.includes("TRIGGER:error-401")) return "error-401";
    if (input.includes("TRIGGER:error-429")) return "error-429";
    if (input.includes("TRIGGER:error-500")) return "error-500";
    if (input.includes("TRIGGER:failed")) return "failed-response";
  } else if (Array.isArray(input)) {
    for (const item of input) {
      if (item.type === "message" && "content" in item) {
        const content = item.content;
        if (typeof content === "string") {
          if (content.includes("TRIGGER:error-400")) return "error-400";
          if (content.includes("TRIGGER:error-401")) return "error-401";
          if (content.includes("TRIGGER:error-429")) return "error-429";
          if (content.includes("TRIGGER:error-500")) return "error-500";
          if (content.includes("TRIGGER:failed")) return "failed-response";
        }
      }
    }
  }

  return null;
}

function countMessages(request: CreateResponseBody): number {
  const input = request.input;
  if (!input || typeof input === "string") return 1;
  return input.filter((item) => item.type === "message").length;
}

// ============================================================================
// Request Handlers
// ============================================================================

async function handleResponses(
  req: IncomingMessage,
  res: ServerResponse,
  config: ServerConfig
): Promise<void> {
  requestCount++;

  // Get request body
  const bodyStr = await getRequestBody(req);

  // Parse JSON
  let rawBody: unknown;
  try {
    rawBody = JSON.parse(bodyStr);
  } catch {
    lastValidationReport = {
      valid: false,
      errors: [{ code: "custom", message: "Invalid JSON", path: [], fatal: true } as unknown as ZodIssue],
      timestamp: Date.now(),
    };
    sendJson(res, 400, {
      error: { code: "invalid_json", message: "Request body is not valid JSON" },
    });
    return;
  }

  // Validate against schema
  const parseResult = createResponseBodySchema.safeParse(rawBody);

  lastValidationReport = {
    valid: parseResult.success,
    errors: parseResult.success ? [] : parseResult.error.issues,
    request: parseResult.success ? parseResult.data : undefined,
    timestamp: Date.now(),
  };

  if (!parseResult.success) {
    if (config.verbose) {
      console.log(`[${requestCount}] Validation failed:`, parseResult.error.issues);
    }
    sendJson(res, 400, {
      error: {
        code: "validation_error",
        message: "Request validation failed",
        details: parseResult.error.issues,
      },
    });
    return;
  }

  const request = parseResult.data;
  const model = request.model || "test-model";
  const isStreaming = request.stream === true;
  const hasTools = request.tools && request.tools.length > 0;

  if (config.verbose) {
    console.log(`[${requestCount}] Valid request - model: ${model}, streaming: ${isStreaming}, tools: ${hasTools}`);
  }

  // Check for error triggers
  const trigger = detectTrigger(request);

  if (trigger === "error-400") {
    sendJson(res, 400, {
      error: { code: "bad_request", message: "Triggered bad request error" },
    });
    return;
  }

  if (trigger === "error-401") {
    sendJson(res, 401, {
      error: { code: "unauthorized", message: "Triggered unauthorized error" },
    });
    return;
  }

  if (trigger === "error-429") {
    res.setHeader("Retry-After", "60");
    sendJson(res, 429, {
      error: { code: "rate_limit_exceeded", message: "Triggered rate limit error" },
    });
    return;
  }

  if (trigger === "error-500") {
    sendJson(res, 500, {
      error: { code: "internal_error", message: "Triggered internal server error" },
    });
    return;
  }

  if (trigger === "failed-response") {
    const response = generateFailedResponse(
      { model, request },
      "content_filter",
      "Triggered failed response status"
    );
    sendJson(res, 200, response);
    return;
  }

  // Handle streaming
  if (isStreaming) {
    if (hasTools) {
      const tool = request.tools![0];
      const events = generateToolCallStreamingEvents(
        { model, request },
        tool.name,
        { location: "San Francisco, CA" }
      );
      sendSSE(res, events);
    } else {
      const events = generateStreamingEvents({ model, request });
      sendSSE(res, events);
    }
    return;
  }

  // Handle tool calls
  if (hasTools) {
    const tool = request.tools![0];
    const response = generateToolCallResponse(
      { model, request },
      tool.name,
      { location: "San Francisco, CA" }
    );
    sendJson(res, 200, response);
    return;
  }

  // Handle multi-turn
  if (countMessages(request) > 1) {
    const response = generateMultiTurnResponse({ model, request });
    sendJson(res, 200, response);
    return;
  }

  // Default: basic response
  const response = generateBasicResponse({ model, request });
  sendJson(res, 200, response);
}

function handleScenarios(res: ServerResponse): void {
  sendJson(res, 200, testScenarios);
}

function handleHealth(res: ServerResponse): void {
  sendJson(res, 200, {
    status: "ok",
    requests_processed: requestCount,
    timestamp: Date.now(),
  });
}

function handleValidationReport(res: ServerResponse): void {
  if (!lastValidationReport) {
    sendJson(res, 404, {
      error: { message: "No requests have been processed yet" },
    });
    return;
  }
  sendJson(res, 200, lastValidationReport);
}

function handleCors(res: ServerResponse): void {
  res.writeHead(204, {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, Authorization, X-API-Key",
    "Access-Control-Max-Age": "86400",
  });
  res.end();
}

// ============================================================================
// Main Server
// ============================================================================

export function createConformanceServer(config: ServerConfig): ReturnType<typeof createServer> {
  const server = createServer(async (req, res) => {
    const url = new URL(req.url || "/", `http://${req.headers.host}`);
    const path = url.pathname;
    const method = req.method || "GET";

    if (config.verbose) {
      console.log(`${method} ${path}`);
    }

    // Handle CORS preflight
    if (method === "OPTIONS") {
      handleCors(res);
      return;
    }

    try {
      if (path === "/responses" && method === "POST") {
        await handleResponses(req, res, config);
      } else if (path === "/scenarios" && method === "GET") {
        handleScenarios(res);
      } else if (path === "/health" && method === "GET") {
        handleHealth(res);
      } else if (path === "/validation-report" && method === "GET") {
        handleValidationReport(res);
      } else {
        sendJson(res, 404, {
          error: { message: `Not found: ${method} ${path}` },
        });
      }
    } catch (error) {
      console.error("Server error:", error);
      sendJson(res, 500, {
        error: {
          code: "internal_error",
          message: error instanceof Error ? error.message : "Unknown error",
        },
      });
    }
  });

  return server;
}

export function startServer(config: ServerConfig): Promise<void> {
  return new Promise((resolve) => {
    const server = createConformanceServer(config);
    server.listen(config.port, config.host, () => {
      console.log(`OpenResponses Client Conformance Server`);
      console.log(`Listening on http://${config.host}:${config.port}`);
      console.log();
      console.log(`Endpoints:`);
      console.log(`  POST /responses         - Main API endpoint`);
      console.log(`  GET  /scenarios         - List test scenarios`);
      console.log(`  GET  /health            - Health check`);
      console.log(`  GET  /validation-report - Last request validation report`);
      console.log();
      resolve();
    });
  });
}
