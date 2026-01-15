# OpenResponses Compliance Test CLI

A Node.js CLI tool for running compliance tests against implementations of the [OpenResponses API](https://github.com/openresponses/openresponses) standard.

## Overview

This tool helps verify that API servers correctly implement the OpenResponses standard by running a suite of compliance tests that check:

- **Schema validation**: Responses match the expected Zod schemas
- **Semantic validation**: Responses contain expected output types and statuses
- **Streaming support**: SSE streaming events are correctly formatted

## Installation

```bash
# Clone or copy this directory
cd compliance-test-cli

# Install dependencies
npm install

# Run directly with tsx
npx tsx src/cli.ts --help

# Or build for production
npm run build
```

## Usage

### List Available Tests

```bash
npx tsx src/cli.ts list
```

Output:
```
Available Compliance Tests:

  basic-response
    Basic Text Response
    Simple user message, validates ResponseResource schema

  streaming-response
    Streaming Response
    Validates SSE streaming events and final response

  system-prompt
    System Prompt
    Include system role message in input

  tool-calling
    Tool Calling
    Define a function tool and verify function_call output

  image-input
    Image Input
    Send image URL in user content

  multi-turn
    Multi-turn Conversation
    Send assistant + user messages as conversation history
```

### Run Compliance Tests

```bash
# Run all tests against a server
npx tsx src/cli.ts run \
  --url https://api.example.com/v1 \
  --api-key sk-your-api-key \
  --model gpt-4o-mini

# Run specific tests only
npx tsx src/cli.ts run \
  --url https://api.example.com/v1 \
  --api-key sk-your-api-key \
  --filter basic-response streaming-response

# Output results as JSON (for CI/CD)
npx tsx src/cli.ts run \
  --url https://api.example.com/v1 \
  --api-key sk-your-api-key \
  --json

# Show verbose output with request/response details
npx tsx src/cli.ts run \
  --url https://api.example.com/v1 \
  --api-key sk-your-api-key \
  --verbose
```

### CLI Options

```
Usage: openresponses-compliance run [options]

Run compliance tests against a server

Options:
  -u, --url <url>             Base URL of the API server (required)
  -k, --api-key <key>         API key for authentication (required)
  -m, --model <model>         Model name to use for tests (default: "gpt-4o-mini")
  -H, --auth-header <header>  Name of the authorization header (default: "Authorization")
  --no-bearer                 Don't use 'Bearer ' prefix for API key
  -t, --timeout <ms>          Request timeout in milliseconds (default: 60000)
  -f, --filter <tests...>     Run only specific test IDs
  -j, --json                  Output results as JSON
  -v, --verbose               Show detailed request/response information
```

## Test Descriptions

| Test ID | Name | Description |
|---------|------|-------------|
| `basic-response` | Basic Text Response | Simple user message, validates ResponseResource schema |
| `streaming-response` | Streaming Response | Validates SSE streaming events and final response |
| `system-prompt` | System Prompt | Include system role message in input |
| `tool-calling` | Tool Calling | Define a function tool and verify function_call output |
| `image-input` | Image Input | Send image URL in user content |
| `multi-turn` | Multi-turn Conversation | Send assistant + user messages as conversation history |

## Exit Codes

- `0`: All tests passed
- `1`: One or more tests failed

## JSON Output Format

When using `--json`, the output follows this structure:

```json
{
  "summary": {
    "total": 6,
    "passed": 5,
    "failed": 1,
    "skipped": 0
  },
  "results": [
    {
      "id": "basic-response",
      "name": "Basic Text Response",
      "description": "Simple user message, validates ResponseResource schema",
      "status": "passed",
      "duration": 1234,
      "errors": [],
      "request": { ... },
      "response": { ... }
    },
    ...
  ]
}
```

## Architecture

The CLI tool is structured as follows:

```
src/
├── cli.ts              # Main CLI entry point using Commander.js
├── compliance-tests.ts # Test templates and test runner
├── schemas.ts          # Simplified Zod schemas for validation
└── sse-parser.ts       # Server-Sent Events stream parser
```

### Key Components

- **schemas.ts**: Contains simplified Zod schemas extracted from the openresponses repository's Kubb-generated schemas. These validate the response structure without requiring the full generated code.

- **sse-parser.ts**: Parses SSE streaming responses, validates each event against the streaming event schemas, and extracts the final response from `response.completed` or `response.failed` events.

- **compliance-tests.ts**: Defines test templates with validators that check both schema compliance and semantic correctness (e.g., ensuring responses have output items, correct status, etc.).

- **cli.ts**: Provides the command-line interface using Commander.js with colored output via Chalk.

## Based On

This CLI tool is based on the compliance tests from the [OpenResponses](https://github.com/openresponses/openresponses) project, specifically:

- `src/lib/compliance-tests.ts`
- `src/lib/sse-parser.ts`
- `src/generated/kubb/zod/*.ts`

## License

MIT
