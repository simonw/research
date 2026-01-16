/**
 * Zod schemas for validating OpenResponses API requests and generating responses.
 * These schemas are used for client conformance testing.
 */

import { z } from "zod";

// ============================================================================
// Request Content Schemas
// ============================================================================

export const inputTextContentParamSchema = z.object({
  type: z.literal("input_text"),
  text: z.string(),
});

export const inputImageContentParamSchema = z.object({
  type: z.literal("input_image"),
  image_url: z.string().optional(),
  detail: z.enum(["auto", "low", "high"]).optional(),
  // Base64 image data
  image: z.string().optional(),
  media_type: z.string().optional(),
});

export const inputFileContentParamSchema = z.object({
  type: z.literal("input_file"),
  file_id: z.string().optional(),
  file_data: z.string().optional(),
  filename: z.string().optional(),
});

export const contentPartParamSchema = z.union([
  inputTextContentParamSchema,
  inputImageContentParamSchema,
  inputFileContentParamSchema,
]);

// ============================================================================
// Message Item Schemas
// ============================================================================

export const userMessageItemParamSchema = z.object({
  id: z.string().optional().nullable(),
  type: z.literal("message").default("message"),
  role: z.literal("user"),
  content: z.union([z.array(contentPartParamSchema), z.string()]),
  status: z.string().optional().nullable(),
});

export const systemMessageItemParamSchema = z.object({
  id: z.string().optional().nullable(),
  type: z.literal("message").default("message"),
  role: z.literal("system"),
  content: z.union([z.array(contentPartParamSchema), z.string()]),
  status: z.string().optional().nullable(),
});

export const developerMessageItemParamSchema = z.object({
  id: z.string().optional().nullable(),
  type: z.literal("message").default("message"),
  role: z.literal("developer"),
  content: z.union([z.array(contentPartParamSchema), z.string()]),
  status: z.string().optional().nullable(),
});

export const outputTextContentParamSchema = z.object({
  type: z.literal("output_text"),
  text: z.string(),
  annotations: z.array(z.any()).optional(),
});

export const refusalContentParamSchema = z.object({
  type: z.literal("refusal"),
  refusal: z.string(),
});

export const assistantMessageItemParamSchema = z.object({
  id: z.string().optional().nullable(),
  type: z.literal("message").default("message"),
  role: z.literal("assistant"),
  content: z.union([
    z.array(z.union([outputTextContentParamSchema, refusalContentParamSchema])),
    z.string(),
  ]),
  status: z.string().optional().nullable(),
});

// ============================================================================
// Function/Tool Schemas
// ============================================================================

export const functionCallItemParamSchema = z.object({
  id: z.string().optional().nullable(),
  type: z.literal("function_call"),
  call_id: z.string(),
  name: z.string(),
  arguments: z.string(),
  status: z.string().optional().nullable(),
});

export const functionCallOutputItemParamSchema = z.object({
  id: z.string().optional().nullable(),
  type: z.literal("function_call_output"),
  call_id: z.string(),
  output: z.string(),
  status: z.string().optional().nullable(),
});

export const functionToolParamSchema = z.object({
  type: z.literal("function"),
  name: z.string(),
  description: z.string().optional(),
  parameters: z.any().optional(),
  strict: z.boolean().optional(),
});

export const toolParamSchema = functionToolParamSchema;

// ============================================================================
// Other Item Schemas
// ============================================================================

export const reasoningItemParamSchema = z.object({
  id: z.string().optional().nullable(),
  type: z.literal("reasoning"),
  summary: z.array(z.any()).optional(),
  encrypted_content: z.string().optional().nullable(),
  status: z.string().optional().nullable(),
});

export const itemReferenceParamSchema = z.object({
  type: z.literal("item_reference"),
  id: z.string(),
});

// Combined item param schema
export const itemParamSchema = z.union([
  itemReferenceParamSchema,
  reasoningItemParamSchema,
  userMessageItemParamSchema,
  systemMessageItemParamSchema,
  developerMessageItemParamSchema,
  assistantMessageItemParamSchema,
  functionCallItemParamSchema,
  functionCallOutputItemParamSchema,
]);

// ============================================================================
// Tool Choice Schemas
// ============================================================================

export const specificFunctionParamSchema = z.object({
  name: z.string(),
});

export const functionToolChoiceSchema = z.object({
  type: z.literal("function"),
  function: specificFunctionParamSchema,
});

export const toolChoiceParamSchema = z.union([
  z.enum(["auto", "none", "required"]),
  functionToolChoiceSchema,
]);

// ============================================================================
// Other Parameter Schemas
// ============================================================================

export const includeEnumSchema = z.enum([
  "file_search_call.results",
  "message.input_image.image_url",
  "computer_call_output.output.image_url",
  "reasoning.encrypted_content",
  "code_interpreter_call.outputs",
]);

export const truncationEnumSchema = z.enum(["auto", "disabled"]);

export const serviceTierEnumSchema = z.enum(["auto", "default", "flex"]);

export const textFormatParamSchema = z.object({
  type: z.enum(["text", "json_object", "json_schema"]),
  json_schema: z.any().optional(),
});

export const textParamSchema = z.object({
  format: textFormatParamSchema.optional(),
});

export const reasoningEffortEnumSchema = z.enum(["low", "medium", "high"]);

export const reasoningParamSchema = z.object({
  effort: reasoningEffortEnumSchema.optional(),
  summary: z.enum(["auto", "concise", "detailed"]).optional(),
});

export const streamOptionsParamSchema = z.object({
  include_usage: z.boolean().optional(),
});

export const metadataParamSchema = z.record(z.string(), z.string());

// ============================================================================
// Create Response Body Schema (Main Request Schema)
// ============================================================================

export const createResponseBodySchema = z.object({
  model: z.string().optional().nullable(),
  input: z.union([z.array(itemParamSchema), z.string()]).optional().nullable(),
  previous_response_id: z.string().optional().nullable(),
  include: z.array(includeEnumSchema).optional(),
  tools: z.array(toolParamSchema).optional().nullable(),
  tool_choice: z.union([toolChoiceParamSchema, z.null()]).optional(),
  metadata: z.union([metadataParamSchema, z.null()]).optional(),
  text: z.union([textParamSchema, z.null()]).optional(),
  temperature: z.number().min(0).max(2).optional().nullable(),
  top_p: z.number().min(0).max(1).optional().nullable(),
  presence_penalty: z.number().min(-2).max(2).optional().nullable(),
  frequency_penalty: z.number().min(-2).max(2).optional().nullable(),
  parallel_tool_calls: z.boolean().optional().nullable(),
  stream: z.boolean().optional(),
  stream_options: z.union([streamOptionsParamSchema, z.null()]).optional(),
  background: z.boolean().optional(),
  max_output_tokens: z.number().int().positive().optional().nullable(),
  max_tool_calls: z.number().int().positive().optional().nullable(),
  reasoning: z.union([reasoningParamSchema, z.null()]).optional(),
  safety_identifier: z.string().optional().nullable(),
  prompt_cache_key: z.string().optional().nullable(),
  truncation: z.union([truncationEnumSchema, z.null()]).optional(),
  instructions: z.string().optional().nullable(),
  store: z.boolean().optional(),
  service_tier: z.union([serviceTierEnumSchema, z.null()]).optional(),
  top_logprobs: z.number().int().min(0).max(20).optional().nullable(),
});

export type CreateResponseBody = z.infer<typeof createResponseBodySchema>;

// ============================================================================
// Response Schemas (for generating mock responses)
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
  role: z.literal("assistant"),
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
  z.object({ type: z.string() }).passthrough(),
]);

export const errorSchema = z.object({
  code: z.string().optional(),
  message: z.string(),
}).passthrough();

export const usageSchema = z.object({
  input_tokens: z.number().int(),
  output_tokens: z.number().int(),
  total_tokens: z.number().int().optional(),
});

export const responseResourceSchema = z.object({
  id: z.string(),
  object: z.literal("response").default("response"),
  created_at: z.number().int(),
  completed_at: z.number().int().optional().nullable(),
  status: z.string(),
  incomplete_details: z.any().optional().nullable(),
  model: z.string(),
  previous_response_id: z.string().optional().nullable(),
  instructions: z.string().optional().nullable(),
  output: z.array(itemFieldSchema),
  error: z.union([errorSchema, z.null()]).optional(),
  tools: z.array(z.any()).optional(),
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
  max_output_tokens: z.number().int().optional().nullable(),
  max_tool_calls: z.number().int().optional().nullable(),
  store: z.boolean().optional(),
  background: z.boolean().optional(),
  service_tier: z.string().optional(),
  metadata: z.any().optional(),
  safety_identifier: z.string().optional().nullable(),
  prompt_cache_key: z.string().optional().nullable(),
}).passthrough();

export type ResponseResource = z.infer<typeof responseResourceSchema>;
