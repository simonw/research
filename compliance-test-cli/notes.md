# OpenResponses Compliance Test CLI - Development Notes

## Investigation Start

Examining the openresponses repository at https://github.com/openresponses/openresponses

### Key Files Analyzed

1. **src/lib/compliance-tests.ts** - Main compliance test implementation
   - Defines `TestConfig` interface for API connection settings
   - Defines `TestTemplate` for test definitions with validators
   - Contains 6 test templates:
     - `basic-response`: Basic text response validation
     - `streaming-response`: SSE streaming validation
     - `system-prompt`: System prompt handling
     - `tool-calling`: Function tool definition and calling
     - `image-input`: Image URL in user content
     - `multi-turn`: Multi-turn conversation history

2. **src/lib/sse-parser.ts** - Server-Sent Events parser
   - Parses SSE streaming responses
   - Validates events against Zod schemas
   - Extracts final response from completed/failed events

3. **src/generated/kubb/zod/** - Generated Zod schemas
   - `responseResourceSchema.ts` - Response object validation
   - `createResponseBodySchema.ts` - Request body validation
   - Many streaming event schemas for SSE validation

### Design Decisions

1. **Standalone CLI Tool**: Will create a self-contained Node.js CLI that doesn't require the full openresponses codebase

2. **Schema Extraction**: Will extract and simplify the necessary Zod schemas to avoid dependency on Kubb-generated code

3. **Features to Include**:
   - Commander.js for CLI argument parsing
   - Colored terminal output for test results
   - JSON output option for CI/CD integration
   - Filter by test ID
   - Configurable timeout
   - Verbose mode for debugging

4. **TypeScript**: Using TypeScript with tsx for execution

## Implementation Progress

### Files Created

1. **package.json** - Project configuration with dependencies:
   - `commander` (v12.1.0) - CLI argument parsing
   - `chalk` (v5.3.0) - Terminal colors
   - `zod` (v3.23.8) - Schema validation
   - `tsx` (v4.15.0) - TypeScript execution
   - `typescript` (v5.4.5) - TypeScript compiler

2. **tsconfig.json** - TypeScript configuration targeting ES2022 with NodeNext modules

3. **src/schemas.ts** - Simplified Zod schemas (450+ lines)
   - Extracted essential schemas from openresponses' Kubb-generated code
   - Added `.passthrough()` to allow additional fields (future-proofing)
   - Includes ResponseResource, streaming events, and output item schemas

4. **src/sse-parser.ts** - SSE stream parser
   - Reads streaming response body using ReadableStream
   - Parses event: and data: lines per SSE spec
   - Validates each event against streaming schema
   - Extracts final response from response.completed/failed events

5. **src/compliance-tests.ts** - Test runner and templates
   - Ported all 6 test templates from original code
   - Validators: hasOutput, hasOutputType, completedStatus, streamingEvents, streamingSchema
   - Sequential test execution with progress callbacks
   - Configurable timeout support

6. **src/cli.ts** - CLI entry point
   - `run` command for executing tests
   - `list` command for showing available tests
   - Colored output with pass/fail status
   - JSON output for CI/CD integration
   - Verbose mode for debugging

### Key Implementation Decisions

1. **Schema Simplification**: Rather than copy all 80+ generated schema files, I created a single schemas.ts that captures the essential validation while using `.passthrough()` to allow extra fields. This makes the tool more resilient to API changes.

2. **Sequential vs Parallel Tests**: Changed from parallel (Promise.all) to sequential test execution. This provides clearer progress output and avoids potential rate limiting issues.

3. **Timeout Support**: Added configurable timeout using AbortController to handle slow or unresponsive servers.

4. **Exit Codes**: Returns 0 for all pass, 1 for any failure - suitable for CI/CD pipelines.

### Testing Results

```bash
$ npx tsx src/cli.ts --help
# Shows version, commands

$ npx tsx src/cli.ts list
# Lists all 6 compliance tests with descriptions

$ npx tsx src/cli.ts list --json
# JSON output of test list

$ npx tsx src/cli.ts run --help
# Shows all run options
```

The CLI tool compiles and runs successfully. Actual API testing would require a valid OpenResponses-compatible server endpoint.

## Lessons Learned

1. The OpenResponses API uses a discriminated union pattern for output items (message, function_call, etc.) which requires careful Zod schema design.

2. SSE parsing requires handling buffered reads and incomplete lines at chunk boundaries.

3. The streaming event types are extensive (22+ event types) covering the full lifecycle of a response generation.

4. Using `.passthrough()` in Zod schemas is important for forward compatibility when the API might add new fields.
