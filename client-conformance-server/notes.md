# OpenResponses Client Conformance Test Server - Development Notes

## Goal

Create a Node.js mock server that tests client implementations of the OpenResponses API by:
1. Validating incoming requests against the OpenResponses schema
2. Returning mock responses in the correct format
3. Supporting streaming responses with proper SSE format
4. Testing various scenarios (success, errors, tool calls, etc.)

## Architecture Design

### Server Components

1. **Request Validator** - Validates incoming POST /responses requests against Zod schemas
2. **Mock Response Generator** - Creates valid OpenResponses responses based on test scenarios
3. **SSE Streamer** - Generates properly formatted SSE streaming responses
4. **Test Scenarios** - Predefined scenarios that clients can trigger to test edge cases

### Test Scenarios

The server will support different test scenarios based on input patterns:

1. **Basic Response** - Returns a simple text response
2. **Streaming Response** - Returns SSE stream with multiple events
3. **Tool Call Response** - Returns function_call output items
4. **Error Response** - Returns various HTTP errors (400, 401, 500, etc.)
5. **Schema Validation Failure** - Reports detailed validation errors for malformed requests
6. **Multi-turn Context** - Verifies conversation history is properly sent
7. **Image Input** - Validates image content in requests

### API Design

```
POST /responses          - Main API endpoint (validates requests, returns mock responses)
GET /scenarios           - List available test scenarios
GET /health              - Health check
GET /validation-report   - Get detailed validation report for last request
```

### Triggering Scenarios

Clients can trigger specific scenarios by:
1. Including special text in the input (e.g., "TRIGGER:error-500")
2. Using specific model names (e.g., "test-streaming", "test-error-401")
3. Query parameters (e.g., ?scenario=streaming)

## Implementation Progress

### Files Created

1. **package.json** - Project config with commander, chalk, zod dependencies
2. **tsconfig.json** - TypeScript configuration
3. **src/schemas.ts** - Comprehensive request validation schemas
   - All input item types (user, system, developer, assistant messages)
   - Function call items and outputs
   - Tool definitions
   - All request parameters (temperature, tools, streaming options, etc.)
4. **src/mock-responses.ts** - Response generators
   - Basic text responses
   - Tool call responses
   - Multi-turn responses
   - Streaming event sequences
   - Failed response generation
5. **src/server.ts** - HTTP server
   - Request validation with Zod
   - Scenario detection from model name or input triggers
   - SSE streaming support
   - CORS headers for browser testing
6. **src/cli.ts** - CLI interface
   - `start` - Start the server
   - `scenarios` - List available scenarios
   - `test-request` - Send test requests

### Test Scenarios Implemented

| Scenario | Trigger | Response |
|----------|---------|----------|
| basic | Default | Simple text response |
| streaming | `stream: true` | Full SSE event sequence |
| tool-call | `tools` array present | function_call output |
| tool-call-streaming | `tools` + `stream` | Streaming tool call events |
| multi-turn | Multiple messages | Echoes message count |
| error-400 | Model "error-400" | HTTP 400 |
| error-401 | Model "error-401" | HTTP 401 |
| error-429 | Model "error-429" | HTTP 429 + Retry-After |
| error-500 | Model "error-500" | HTTP 500 |
| failed-response | Model "failed" | status: "failed" |

### Testing Results

Successfully tested:
- Basic request/response flow
- Request validation with detailed error messages
- Tool call responses
- Full SSE streaming with all event types
- Error triggers (401, 500, etc.)

Example streaming output:
```
event: response.created
data: {"type":"response.created","sequence_number":0,...}

event: response.output_text.delta
data: {"type":"response.output_text.delta","delta":"Hello",...}
...
event: response.completed
data: {"type":"response.completed","response":{...}}

data: [DONE]
```

## Key Design Decisions

1. **Built-in HTTP module**: Used Node.js `http` module instead of Express to minimize dependencies.

2. **Trigger patterns**: Multiple ways to trigger scenarios:
   - Model name (e.g., `model: "error-401"`)
   - Input content (e.g., `"TRIGGER:error-500"`)
   - Request structure (e.g., tools array triggers tool call response)

3. **Validation report endpoint**: `/validation-report` allows clients to inspect detailed validation results for debugging.

4. **Streaming delays**: Added 50ms delay between SSE events to simulate realistic streaming behavior.

## Lessons Learned

1. The request schema is more complex than the response schema - many discriminated unions for different item types.

2. SSE format requires careful handling: `event:` line, `data:` line, double newline separator.

3. Zod union errors can be verbose - the validation error for an invalid role shows all possible valid alternatives.

4. CORS headers are essential for browser-based client testing.
