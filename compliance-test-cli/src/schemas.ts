/**
 * Simplified Zod schemas for OpenResponses API compliance testing.
 * These are extracted and simplified from the openresponses repository's
 * Kubb-generated schemas.
 */

import { z } from "zod";

// ============================================================================
// Output Item Schemas
// ============================================================================

export const outputTextContentSchema = z.object({
  type: z.literal("output_text"),
  text: z.string(),
  annotations: z.array(z.any()).optional(),
});

export const refusalContentSchema = z.object({
  type: z.literal("refusal"),
  refusal: z.string(),
});

export const messageSchema = z.object({
  type: z.literal("message"),
  id: z.string(),
  role: z.enum(["assistant"]),
  status: z.string(),
  content: z.array(z.union([outputTextContentSchema, refusalContentSchema, z.any()])),
});

export const functionCallSchema = z.object({
  type: z.literal("function_call"),
  id: z.string(),
  call_id: z.string(),
  name: z.string(),
  arguments: z.string(),
  status: z.string().optional(),
});

export const functionCallOutputSchema = z.object({
  type: z.literal("function_call_output"),
  call_id: z.string(),
  output: z.string(),
  status: z.string().optional(),
});

export const reasoningSchema = z.object({
  type: z.literal("reasoning"),
  id: z.string(),
  summary: z.array(z.any()).optional(),
  encrypted_content: z.string().optional(),
  status: z.string().optional(),
});

export const itemFieldSchema = z.union([
  messageSchema,
  functionCallSchema,
  functionCallOutputSchema,
  reasoningSchema,
  z.object({ type: z.string() }).passthrough(), // Allow other item types
]);

// ============================================================================
// Tool Schemas
// ============================================================================

export const functionToolSchema = z.object({
  type: z.literal("function"),
  name: z.string(),
  description: z.string().optional(),
  parameters: z.any().optional(),
  strict: z.boolean().optional(),
});

export const toolSchema = z.union([
  functionToolSchema,
  z.object({ type: z.string() }).passthrough(), // Allow other tool types
]);

// ============================================================================
// Response Resource Schema
// ============================================================================

export const incompleteDetailsSchema = z.object({
  reason: z.string().optional(),
}).passthrough();

export const errorSchema = z.object({
  code: z.string().optional(),
  message: z.string(),
}).passthrough();

export const usageSchema = z.object({
  input_tokens: z.number().int(),
  output_tokens: z.number().int(),
  total_tokens: z.number().int().optional(),
  input_tokens_details: z.any().optional(),
  output_tokens_details: z.any().optional(),
}).passthrough();

export const responseResourceSchema = z.object({
  id: z.string(),
  object: z.enum(["response"]).default("response"),
  created_at: z.number().int(),
  completed_at: z.union([z.number().int(), z.null()]).optional(),
  status: z.string(),
  incomplete_details: z.union([incompleteDetailsSchema, z.null()]).optional(),
  model: z.string(),
  previous_response_id: z.union([z.string(), z.null()]).optional(),
  instructions: z.union([z.string(), z.null()]).optional(),
  output: z.array(itemFieldSchema),
  error: z.union([errorSchema, z.null()]).optional(),
  tools: z.array(toolSchema).optional(),
  tool_choice: z.any().optional(),
  truncation: z.any().optional(),
  parallel_tool_calls: z.boolean().optional(),
  text: z.any().optional(),
  top_p: z.number().optional(),
  presence_penalty: z.number().optional(),
  frequency_penalty: z.number().optional(),
  top_logprobs: z.number().int().optional(),
  temperature: z.number().optional(),
  reasoning: z.any().optional(),
  usage: z.union([usageSchema, z.null()]).optional(),
  max_output_tokens: z.union([z.number().int(), z.null()]).optional(),
  max_tool_calls: z.union([z.number().int(), z.null()]).optional(),
  store: z.boolean().optional(),
  background: z.boolean().optional(),
  service_tier: z.string().optional(),
  metadata: z.any().optional(),
  safety_identifier: z.union([z.string(), z.null()]).optional(),
  prompt_cache_key: z.union([z.string(), z.null()]).optional(),
}).passthrough();

export type ResponseResource = z.infer<typeof responseResourceSchema>;

// ============================================================================
// Streaming Event Schemas
// ============================================================================

const baseStreamingEventSchema = z.object({
  sequence_number: z.number().int().optional(),
});

export const responseCreatedStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.created"),
  response: responseResourceSchema,
});

export const responseQueuedStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.queued"),
  response: responseResourceSchema,
});

export const responseInProgressStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.in_progress"),
  response: responseResourceSchema,
});

export const responseCompletedStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.completed"),
  response: responseResourceSchema,
});

export const responseFailedStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.failed"),
  response: responseResourceSchema,
});

export const responseIncompleteStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.incomplete"),
  response: responseResourceSchema,
});

export const responseOutputItemAddedStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.output_item.added"),
  output_index: z.number().int(),
  item: itemFieldSchema,
});

export const responseOutputItemDoneStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.output_item.done"),
  output_index: z.number().int(),
  item: itemFieldSchema,
});

export const responseContentPartAddedStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.content_part.added"),
  item_id: z.string(),
  output_index: z.number().int(),
  content_index: z.number().int(),
  part: z.any(),
});

export const responseContentPartDoneStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.content_part.done"),
  item_id: z.string(),
  output_index: z.number().int(),
  content_index: z.number().int(),
  part: z.any(),
});

export const responseOutputTextDeltaStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.output_text.delta"),
  item_id: z.string(),
  output_index: z.number().int(),
  content_index: z.number().int(),
  delta: z.string(),
});

export const responseOutputTextDoneStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.output_text.done"),
  item_id: z.string(),
  output_index: z.number().int(),
  content_index: z.number().int(),
  text: z.string(),
});

export const responseRefusalDeltaStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.refusal.delta"),
  item_id: z.string(),
  output_index: z.number().int(),
  content_index: z.number().int(),
  delta: z.string(),
});

export const responseRefusalDoneStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.refusal.done"),
  item_id: z.string(),
  output_index: z.number().int(),
  content_index: z.number().int(),
  refusal: z.string(),
});

export const responseFunctionCallArgumentsDeltaStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.function_call_arguments.delta"),
  item_id: z.string(),
  output_index: z.number().int(),
  call_id: z.string(),
  delta: z.string(),
});

export const responseFunctionCallArgumentsDoneStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.function_call_arguments.done"),
  item_id: z.string(),
  output_index: z.number().int(),
  call_id: z.string(),
  arguments: z.string(),
});

export const responseReasoningDeltaStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.reasoning.delta"),
  item_id: z.string(),
  output_index: z.number().int(),
  content_index: z.number().int().optional(),
  delta: z.string(),
});

export const responseReasoningDoneStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.reasoning.done"),
  item_id: z.string(),
  output_index: z.number().int(),
  content_index: z.number().int().optional(),
  text: z.string(),
});

export const responseReasoningSummaryPartAddedStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.reasoning_summary_part.added"),
  item_id: z.string(),
  output_index: z.number().int(),
  summary_index: z.number().int(),
  part: z.any(),
});

export const responseReasoningSummaryPartDoneStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.reasoning_summary_part.done"),
  item_id: z.string(),
  output_index: z.number().int(),
  summary_index: z.number().int(),
  part: z.any(),
});

export const responseReasoningSummaryDeltaStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.reasoning_summary.delta"),
  item_id: z.string(),
  output_index: z.number().int(),
  summary_index: z.number().int(),
  delta: z.string(),
});

export const responseReasoningSummaryDoneStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.reasoning_summary.done"),
  item_id: z.string(),
  output_index: z.number().int(),
  summary_index: z.number().int(),
  text: z.string(),
});

export const responseOutputTextAnnotationAddedStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("response.output_text.annotation.added"),
  item_id: z.string(),
  output_index: z.number().int(),
  content_index: z.number().int(),
  annotation_index: z.number().int(),
  annotation: z.any(),
});

export const errorStreamingEventSchema = baseStreamingEventSchema.extend({
  type: z.literal("error"),
  error: errorSchema,
});

export const streamingEventSchema = z.union([
  responseCreatedStreamingEventSchema,
  responseQueuedStreamingEventSchema,
  responseInProgressStreamingEventSchema,
  responseCompletedStreamingEventSchema,
  responseFailedStreamingEventSchema,
  responseIncompleteStreamingEventSchema,
  responseOutputItemAddedStreamingEventSchema,
  responseOutputItemDoneStreamingEventSchema,
  responseContentPartAddedStreamingEventSchema,
  responseContentPartDoneStreamingEventSchema,
  responseOutputTextDeltaStreamingEventSchema,
  responseOutputTextDoneStreamingEventSchema,
  responseRefusalDeltaStreamingEventSchema,
  responseRefusalDoneStreamingEventSchema,
  responseFunctionCallArgumentsDeltaStreamingEventSchema,
  responseFunctionCallArgumentsDoneStreamingEventSchema,
  responseReasoningDeltaStreamingEventSchema,
  responseReasoningDoneStreamingEventSchema,
  responseReasoningSummaryPartAddedStreamingEventSchema,
  responseReasoningSummaryPartDoneStreamingEventSchema,
  responseReasoningSummaryDeltaStreamingEventSchema,
  responseReasoningSummaryDoneStreamingEventSchema,
  responseOutputTextAnnotationAddedStreamingEventSchema,
  errorStreamingEventSchema,
]);

export type StreamingEvent = z.infer<typeof streamingEventSchema>;
