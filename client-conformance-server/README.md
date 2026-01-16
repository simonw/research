# OpenResponses Client Conformance Server

A Node.js mock server for testing client implementations of the [OpenResponses API](https://github.com/openresponses/openresponses) standard.

## Overview

While the OpenResponses project provides compliance tests for **server** implementations, this tool provides conformance tests for **client** implementations. It works by:

1. **Validating incoming requests** against the OpenResponses request schema
2. **Returning mock responses** that conform to the response schema
3. **Supporting various test scenarios** including errors, streaming, and tool calls

This allows client library developers to verify their implementation correctly:
- Constructs valid request payloads
- Handles response objects correctly
- Processes SSE streaming events properly
- Handles error responses appropriately

## Installation

```bash
# Clone or copy this directory
cd client-conformance-server

# Install dependencies
npm install

# Run with tsx
npx tsx src/cli.ts start
```

## Usage

### Start the Server

```bash
# Start on default port 8080
npx tsx src/cli.ts start

# Start on custom port with verbose logging
npx tsx src/cli.ts start -p 3000 -v
```

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/responses` | POST | Main API endpoint - validates requests, returns mock responses |
| `/scenarios` | GET | List available test scenarios |
| `/health` | GET | Health check |
| `/validation-report` | GET | Get validation report for the last request |

### List Test Scenarios

```bash
npx tsx src/cli.ts scenarios
```

### Send Test Requests

```bash
# Basic test request
npx tsx src/cli.ts test-request

# Test with streaming
npx tsx src/cli.ts test-request --stream

# Trigger specific scenario
npx tsx src/cli.ts test-request --scenario error-401
```

## Test Scenarios

The server supports multiple test scenarios that can be triggered by:
- Model name patterns (e.g., `model: "error-401"`)
- Input content triggers (e.g., `"TRIGGER:error-500"`)
- Request structure (e.g., including `tools` array)

| Scenario ID | Name | Trigger |
|-------------|------|---------|
| `basic` | Basic Response | Default behavior |
| `streaming` | Streaming Response | Set `stream: true` |
| `tool-call` | Tool Call Response | Include `tools` array |
| `tool-call-streaming` | Tool Call Streaming | `tools` + `stream: true` |
| `multi-turn` | Multi-turn Context | Multiple messages in input |
| `error-400` | Bad Request | Model contains "error-400" |
| `error-401` | Unauthorized | Model contains "error-401" |
| `error-429` | Rate Limited | Model contains "error-429" |
| `error-500` | Server Error | Model contains "error-500" |
| `failed-response` | Failed Status | Model contains "failed" |

## Example Requests

### Basic Request

```bash
curl -X POST http://localhost:8080/responses \
  -H "Content-Type: application/json" \
  -d '{
    "model": "test-model",
    "input": [{"type": "message", "role": "user", "content": "Hello"}]
  }'
```

Response:
```json
{
  "id": "resp_123",
  "object": "response",
  "created_at": 1234567890,
  "completed_at": 1234567891,
  "status": "completed",
  "model": "test-model",
  "output": [{
    "type": "message",
    "id": "msg_1",
    "role": "assistant",
    "status": "completed",
    "content": [{
      "type": "output_text",
      "text": "Hello! This is a mock response..."
    }]
  }],
  "usage": {"input_tokens": 10, "output_tokens": 15, "total_tokens": 25}
}
```

### Streaming Request

```bash
curl -X POST http://localhost:8080/responses \
  -H "Content-Type: application/json" \
  -d '{"model": "test", "input": "Hello", "stream": true}'
```

Returns SSE events:
```
event: response.created
data: {"type":"response.created",...}

event: response.output_text.delta
data: {"type":"response.output_text.delta","delta":"Hello",...}

...

event: response.completed
data: {"type":"response.completed","response":{...}}

data: [DONE]
```

### Tool Call Request

```bash
curl -X POST http://localhost:8080/responses \
  -H "Content-Type: application/json" \
  -d '{
    "model": "test",
    "input": [{"type": "message", "role": "user", "content": "Weather?"}],
    "tools": [{
      "type": "function",
      "name": "get_weather",
      "parameters": {"type": "object", "properties": {"location": {"type": "string"}}}
    }]
  }'
```

### Trigger Error Response

```bash
# Using model name
curl -X POST http://localhost:8080/responses \
  -H "Content-Type: application/json" \
  -d '{"model": "error-401", "input": "test"}'

# Using input trigger
curl -X POST http://localhost:8080/responses \
  -H "Content-Type: application/json" \
  -d '{"model": "test", "input": "TRIGGER:error-500"}'
```

## Request Validation

The server validates all incoming requests against the OpenResponses schema. Invalid requests return HTTP 400 with detailed validation errors:

```json
{
  "error": {
    "code": "validation_error",
    "message": "Request validation failed",
    "details": [
      {"path": ["input", 0, "role"], "message": "Invalid literal value, expected \"user\""}
    ]
  }
}
```

You can retrieve the full validation report for the last request:

```bash
curl http://localhost:8080/validation-report
```

## Architecture

```
src/
├── cli.ts            # CLI entry point
├── server.ts         # HTTP server and request handlers
├── schemas.ts        # Zod schemas for request validation
└── mock-responses.ts # Mock response generators
```

### Key Components

- **schemas.ts**: Comprehensive Zod schemas for validating `POST /responses` request bodies, including all input item types, tools, and parameters.

- **mock-responses.ts**: Generators for creating valid OpenResponses responses, including basic responses, tool calls, and streaming events.

- **server.ts**: HTTP server using Node.js built-in `http` module. Handles request routing, validation, scenario detection, and response generation.

## Use Cases

1. **Client Library Development**: Test your OpenResponses client implementation against a compliant server without needing API credentials.

2. **CI/CD Integration**: Run automated tests against the mock server to verify client behavior before deployment.

3. **Edge Case Testing**: Use error triggers to test how your client handles various failure modes.

4. **Streaming Implementation**: Verify correct SSE parsing with realistic streaming events.

## Based On

This server validates requests against schemas derived from the [OpenResponses](https://github.com/openresponses/openresponses) project's Kubb-generated Zod schemas.

## License

MIT
