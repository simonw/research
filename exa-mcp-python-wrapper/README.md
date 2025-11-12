# Exa Code Search - Python Wrapper

A minimal, well-documented Python wrapper for Exa's code context search API. Search billions of GitHub repos, documentation pages, and Stack Overflow posts to find relevant code snippets, examples, and documentation.

## Features

- **Zero external dependencies** - Uses only Python standard library
- **Minimal codebase** - ~170 lines including comprehensive documentation
- **Well-documented** - Full docstrings for all classes and methods
- **Simple API** - Both programmatic and CLI interfaces
- **Type hints** - Full type annotations for better IDE support
- **Robust error handling** - Clear error messages and validation

## Installation

No installation required! Just download `exa_code.py` and set your API key.

```bash
# Clone or download the script
curl -O https://raw.githubusercontent.com/[your-repo]/exa_code.py

# Or just copy exa_code.py to your project
```

## Quick Start

### Set API Key

```bash
export EXA_API_KEY=your_api_key_here
```

### Command Line Usage

```bash
# Basic search with default token count (5000)
python exa_code.py "Python async/await examples"

# Specify custom token count (1000-50000)
python exa_code.py "React hooks useState" 3000

# Use dynamic token adjustment
python exa_code.py "Rust ownership examples" dynamic
```

### Programmatic Usage

```python
from exa_code import ExaCodeSearch

# Initialize with API key from environment
exa = ExaCodeSearch()

# Or pass API key explicitly
exa = ExaCodeSearch(api_key="your_api_key_here")

# Search for code
result = exa.search("FastAPI dependency injection", tokens=4000)

# Access results
print(result['response'])           # Code snippets and documentation
print(result['resultsCount'])       # Number of sources found
print(result['costDollars'])        # API call cost
print(result['searchTime'])         # Search duration in ms
print(result['requestId'])          # Unique request ID

# Optional fields
if 'repository' in result:
    print(result['repository'])     # Source repository info
if 'outputTokens' in result:
    print(result['outputTokens'])   # Output token count
```

## API Reference

### `ExaCodeSearch` Class

#### `__init__(api_key: Optional[str] = None)`

Initialize the Exa code search client.

**Parameters:**
- `api_key` (str, optional): Exa API key. If not provided, reads from `EXA_API_KEY` environment variable.

**Raises:**
- `ValueError`: If no API key is provided or found in environment.

#### `search(query: str, tokens: Union[int, str] = 5000, timeout: int = 30) -> Dict[str, Any]`

Search for code snippets, examples, and documentation.

**Parameters:**
- `query` (str): Natural language search query (e.g., "async/await patterns in Python")
- `tokens` (int | str): Maximum tokens in response. Either:
  - int: 1000-50000 (default: 5000)
  - "dynamic": Auto-adjust token count based on content
- `timeout` (int): Request timeout in seconds (default: 30)

**Returns:**
Dictionary containing:
- `response` (str): Code snippets and documentation
- `resultsCount` (int): Number of sources found
- `costDollars` (str): API call cost
- `searchTime` (float): Search duration in milliseconds
- `requestId` (str): Unique request identifier
- `query` (str): Original search query
- `outputTokens` (int, optional): Number of output tokens
- `repository` (str, optional): Source repository information

**Raises:**
- `ValueError`: If tokens is invalid (not 1000-50000 or "dynamic")
- `urllib.error.HTTPError`: If API request fails
- `json.JSONDecodeError`: If response is not valid JSON

## Examples

### Basic Code Search

```python
from exa_code import ExaCodeSearch

exa = ExaCodeSearch()
result = exa.search("Python async/await examples")
print(result['response'])
```

### Framework-Specific Search

```python
# Search for React patterns
result = exa.search("React hooks useState and useEffect", tokens=3000)

# Search for backend frameworks
result = exa.search("Django REST framework serializers", tokens=4000)

# Search for Rust examples
result = exa.search("Rust ownership and borrowing", tokens=2000)
```

### Error Handling

```python
from exa_code import ExaCodeSearch
from urllib.error import HTTPError

try:
    exa = ExaCodeSearch()
    result = exa.search("Python decorators", tokens=3000)
    print(result['response'])
except ValueError as e:
    print(f"Invalid input: {e}")
except HTTPError as e:
    print(f"API error: {e.code} - {e.msg}")
except Exception as e:
    print(f"Unexpected error: {e}")
```

### Integration with AI Workflows

```python
from exa_code import ExaCodeSearch

def get_code_context(query: str, max_tokens: int = 5000) -> str:
    """Fetch code context for AI assistants."""
    exa = ExaCodeSearch()
    result = exa.search(query, tokens=max_tokens)

    return f"""
Query: {result['query']}
Sources: {result['resultsCount']}
Search Time: {result['searchTime']}ms

{result['response']}
"""

# Use in your AI pipeline
context = get_code_context("Python type hints for async functions")
# Pass context to your LLM...
```

## Development

### Project Structure

```
exa-mcp-python-wrapper/
├── exa_code.py           # Main Python wrapper
├── README.md             # This file
├── notes.md              # Development notes
├── exa-mcp-server/       # Cloned reference (not included in commits)
└── mcp2py/               # Cloned reference (not included in commits)
```

### Testing

```bash
# Test basic search
EXA_API_KEY=your_key python exa_code.py "test query"

# Test with different token counts
EXA_API_KEY=your_key python exa_code.py "test query" 2000

# Test dynamic tokens
EXA_API_KEY=your_key python exa_code.py "test query" dynamic
```

## Implementation Details

### Design Decisions

1. **Standard Library Only**: Uses `urllib` instead of `requests` to avoid external dependencies
2. **Minimal Code**: ~170 lines total, focusing on core functionality
3. **Direct API Calls**: Bypasses MCP layer for simplicity (mcp2py would add ~2600 lines overhead)
4. **Type Safety**: Full type hints for better IDE support and error prevention
5. **Documentation First**: Comprehensive docstrings following Google style

### API Endpoint

- **URL**: `https://api.exa.ai/context`
- **Method**: POST
- **Authentication**: `x-api-key` header
- **Content-Type**: `application/json`

### Request Format

```json
{
  "query": "your search query",
  "tokensNum": 5000
}
```

### Response Format

```json
{
  "requestId": "unique-request-id",
  "query": "original query",
  "response": "code snippets and documentation...",
  "resultsCount": 123,
  "costDollars": {"total": 1, "search": {"neural": 1}},
  "searchTime": 1234.56,
  "outputTokens": 4567,
  "repository": "source info"
}
```

## Research Process

This wrapper was developed by:

1. **Exploring exa-mcp-server** - Analyzed TypeScript MCP implementation to understand the API
2. **Evaluating mcp2py** - Considered using the MCP-to-Python converter but deemed it overkill
3. **Direct API Implementation** - Built minimal wrapper with direct HTTP calls
4. **Self-Improvement** - Used the tool itself to search Exa docs and discovered the "dynamic" token option
5. **Testing & Validation** - Verified with multiple queries across different languages/frameworks

## Comparison: This Wrapper vs. Alternatives

### vs. exa-py SDK
- **exa-py**: General-purpose search, NO code context endpoint
- **This wrapper**: Direct access to `/context` endpoint for code search

### vs. mcp2py
- **mcp2py**: ~2600 lines, subprocess management, full MCP protocol
- **This wrapper**: ~170 lines, direct HTTP, single purpose

### vs. exa-mcp-server
- **exa-mcp-server**: TypeScript, requires Node.js, MCP protocol overhead
- **This wrapper**: Pure Python, standard library only, minimal overhead

## Performance

Typical performance metrics:

- **Search Time**: 2-3 seconds
- **Results**: 100-300 sources per query
- **Cost**: ~$0.005 per query (neural search)
- **Token Range**: 1000-50000 tokens per response

## Limitations

1. **No async support** - Uses synchronous urllib (can be extended with aiohttp)
2. **No caching** - Each query hits the API (can be added with functools.lru_cache)
3. **No retry logic** - Single attempt per request (can be added if needed)
4. **No rate limiting** - No built-in rate limiting (manage externally if needed)

These limitations keep the code minimal while still being fully functional.

## Future Enhancements

Potential additions (not implemented to maintain minimalism):

- Async/await support with aiohttp
- Built-in caching with TTL
- Retry logic with exponential backoff
- Rate limiting
- Response streaming for large results
- Batch query support

## Contributing

This is a minimal reference implementation. Feel free to fork and extend for your needs!

## License

MIT License - Feel free to use in your projects.

## Acknowledgments

- **Exa Labs** - For the excellent code search API
- **exa-mcp-server** - Reference implementation
- **mcp2py** - Inspiration for Python MCP integration

## Resources

- [Exa API Documentation](https://docs.exa.ai/)
- [Exa Code Context Endpoint](https://docs.exa.ai/reference/context)
- [exa-mcp-server Repository](https://github.com/exa-labs/exa-mcp-server)
- [mcp2py Repository](https://github.com/MaximeRivest/mcp2py)
