#!/usr/bin/env python3
"""
Exa Code Search - Minimal Python Wrapper

Direct implementation of Exa's code context search API endpoint.
Searches billions of GitHub repos, documentation pages, and Stack Overflow posts
to find relevant code snippets, examples, and documentation.

API Documentation: https://api.exa.ai/context
"""

import os
import sys
import ssl
from typing import Optional, Dict, Any, Union
from urllib import request, error
from json import dumps, loads


class ExaCodeSearch:
    """Minimal wrapper for Exa's code context search API."""

    API_URL = "https://api.exa.ai/context"
    DEFAULT_TOKENS = 5000
    MIN_TOKENS = 1000
    MAX_TOKENS = 50000

    def __init__(self, api_key: Optional[str] = None):
        """
        Initialize Exa Code Search client.

        Args:
            api_key: Exa API key. If None, reads from EXA_API_KEY environment variable.

        Raises:
            ValueError: If no API key is provided or found in environment.
        """
        self.api_key = api_key or os.getenv("EXA_API_KEY")
        if not self.api_key:
            raise ValueError(
                "API key required. Pass api_key parameter or set EXA_API_KEY environment variable."
            )

    def search(
        self,
        query: str,
        tokens: Union[int, str] = DEFAULT_TOKENS,
        timeout: int = 30
    ) -> Dict[str, Any]:
        """
        Search for code snippets, examples, and documentation.

        Args:
            query: Natural language search query (e.g., "async/await patterns in Python")
            tokens: Maximum tokens in response. Either:
                    - int: 1000-50000 (default: 5000)
                    - "dynamic": Auto-adjust token count based on content
            timeout: Request timeout in seconds (default: 30)

        Returns:
            Dictionary containing:
                - response (str): Code snippets and documentation
                - resultsCount (int): Number of sources found
                - costDollars (str): API call cost
                - searchTime (float): Search duration in milliseconds
                - requestId (str): Unique request identifier
                - query (str): Original search query
                - outputTokens (int, optional): Number of output tokens
                - repository (str, optional): Source repository information

        Raises:
            ValueError: If tokens is invalid (not 1000-50000 or "dynamic")
            urllib.error.HTTPError: If API request fails
            json.JSONDecodeError: If response is not valid JSON

        Example:
            >>> exa = ExaCodeSearch()
            >>> # Fixed token count
            >>> result = exa.search("React useState hook examples", tokens=3000)
            >>> # Dynamic token count
            >>> result = exa.search("Python type hints", tokens="dynamic")
            >>> print(result['response'])
            >>> print(f"Found {result['resultsCount']} sources in {result['searchTime']}ms")
        """
        # Validate parameters
        if not query or not query.strip():
            raise ValueError("Query cannot be empty")

        if isinstance(tokens, str):
            if tokens != "dynamic":
                raise ValueError('Token string must be "dynamic"')
        elif isinstance(tokens, int):
            if not self.MIN_TOKENS <= tokens <= self.MAX_TOKENS:
                raise ValueError(
                    f"Tokens must be between {self.MIN_TOKENS} and {self.MAX_TOKENS} or 'dynamic'"
                )
        else:
            raise ValueError(f"Tokens must be int or 'dynamic', got {type(tokens).__name__}")

        # Build request
        payload = dumps({"query": query.strip(), "tokensNum": tokens}).encode("utf-8")

        headers = {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "x-api-key": self.api_key
        }

        req = request.Request(
            self.API_URL,
            data=payload,
            headers=headers,
            method="POST"
        )

        # Execute request with SSL context
        try:
            context = ssl.create_default_context()
            with request.urlopen(req, timeout=timeout, context=context) as response:
                return loads(response.read().decode("utf-8"))
        except error.HTTPError as e:
            error_body = e.read().decode("utf-8") if e.fp else "No error details"
            raise error.HTTPError(
                e.url,
                e.code,
                f"API request failed: {e.reason}. Details: {error_body}",
                e.headers,
                e.fp
            )


def main():
    """
    Command-line interface for Exa code search.

    Usage:
        python exa_code.py "your search query" [tokens]

    Environment:
        EXA_API_KEY: Required API key for authentication

    Examples:
        python exa_code.py "async/await in Python"
        python exa_code.py "React hooks useState" 3000
        python exa_code.py "Python type hints" dynamic
    """
    if len(sys.argv) < 2:
        print("Usage: python exa_code.py <query> [tokens]", file=sys.stderr)
        print("  tokens: integer (1000-50000) or 'dynamic' (default: 5000)", file=sys.stderr)
        print("Example: python exa_code.py 'React useState' 3000", file=sys.stderr)
        print("Example: python exa_code.py 'Python async' dynamic", file=sys.stderr)
        sys.exit(1)

    query = sys.argv[1]
    tokens = ExaCodeSearch.DEFAULT_TOKENS
    if len(sys.argv) > 2:
        tokens = sys.argv[2] if sys.argv[2] == "dynamic" else int(sys.argv[2])

    try:
        exa = ExaCodeSearch()
        result = exa.search(query, tokens)

        # Display results
        print(f"Query: {result['query']}")
        print(f"Results: {result['resultsCount']} sources")
        print(f"Time: {result['searchTime']:.2f}ms")
        print(f"Cost: ${result['costDollars']}")
        if 'repository' in result:
            print(f"Repository: {result['repository']}")
        print(f"\n{'='*80}\n")
        print(result['response'])

    except ValueError as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
    except error.HTTPError as e:
        print(f"API Error ({e.code}): {e.msg}", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Unexpected error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
