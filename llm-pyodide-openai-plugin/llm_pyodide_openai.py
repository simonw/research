"""
LLM plugin for OpenAI API that works in pyodide/browser environments.

This plugin uses pyodide's fetch API instead of httpx/requests to make
HTTP calls to the OpenAI API from within a browser context.
"""

import json
from typing import Optional, Iterator
try:
    from llm import KeyModel, hookimpl
    from llm.utils import remove_dict_none_values
    from pydantic import Field
except ImportError:
    # Allow this file to be read even if LLM isn't installed
    def hookimpl(func):
        return func
    class KeyModel:
        pass
    class Field:
        pass
    def remove_dict_none_values(d):
        return d


# Try to import pyodide fetch, fall back to None if not in pyodide
try:
    from pyodide.ffi import to_js
    from js import fetch, Headers, Object
    import asyncio
    PYODIDE_AVAILABLE = True
except ImportError:
    PYODIDE_AVAILABLE = False
    to_js = None
    fetch = None
    Headers = None
    Object = None
    asyncio = None


@hookimpl
def register_models(register):
    """Register pyodide-compatible OpenAI models."""
    register(
        PyodideChat("gpt-4o-mini"),
        aliases=("4o-mini", "pyodide-gpt-4o-mini"),
    )
    register(
        PyodideChat("gpt-4o"),
        aliases=("4o", "pyodide-gpt-4o"),
    )
    register(
        PyodideChat("gpt-3.5-turbo"),
        aliases=("3.5", "pyodide-chatgpt"),
    )


class PyodideOptions:
    """Options for PyodideChat model."""
    temperature: Optional[float] = Field(
        description="Sampling temperature between 0 and 2",
        ge=0,
        le=2,
        default=None,
    )
    max_tokens: Optional[int] = Field(
        description="Maximum number of tokens to generate",
        default=None,
    )


class PyodideChat(KeyModel):
    """
    OpenAI Chat model that uses pyodide fetch API for browser compatibility.

    This model makes direct HTTP calls to the OpenAI API using the browser's
    fetch API, which is accessible through pyodide.
    """

    needs_key = "openai"
    key_env_var = "OPENAI_API_KEY"
    can_stream = False  # Streaming not supported in this implementation

    Options = PyodideOptions

    def __init__(self, model_id: str):
        self.model_id = model_id

    def __str__(self):
        return f"Pyodide OpenAI Chat: {self.model_id}"

    def build_messages(self, prompt, conversation):
        """Build messages array for OpenAI API."""
        messages = []

        # Add conversation history
        if conversation is not None:
            for prev_response in conversation.responses:
                if prev_response.prompt.system:
                    messages.append({
                        "role": "system",
                        "content": prev_response.prompt.system
                    })
                if prev_response.prompt.prompt:
                    messages.append({
                        "role": "user",
                        "content": prev_response.prompt.prompt
                    })
                prev_text = prev_response.text()
                if prev_text:
                    messages.append({
                        "role": "assistant",
                        "content": prev_text
                    })

        # Add current prompt
        if prompt.system:
            messages.append({
                "role": "system",
                "content": prompt.system
            })
        if prompt.prompt:
            messages.append({
                "role": "user",
                "content": prompt.prompt
            })

        return messages

    def execute(self, prompt, stream, response, conversation=None, key=None):
        """Execute the prompt using pyodide fetch API."""
        if not PYODIDE_AVAILABLE:
            raise RuntimeError(
                "This plugin requires pyodide environment. "
                "Cannot use browser fetch API outside of pyodide."
            )

        # Get API key
        api_key = self.get_key(key)

        # Build request
        messages = self.build_messages(prompt, conversation)

        # Build kwargs from options
        kwargs = {}
        if prompt.options.temperature is not None:
            kwargs["temperature"] = prompt.options.temperature
        if prompt.options.max_tokens is not None:
            kwargs["max_tokens"] = prompt.options.max_tokens

        payload = {
            "model": self.model_id,
            "messages": messages,
            **kwargs
        }

        # Make the request using pyodide fetch
        result = self._fetch_completion(api_key, payload)

        # Extract response text
        content = result["choices"][0]["message"]["content"]

        # Set usage information
        if "usage" in result:
            usage = result["usage"]
            response.set_usage(
                input=usage.get("prompt_tokens", 0),
                output=usage.get("completion_tokens", 0),
            )

        # Store response JSON
        response.response_json = remove_dict_none_values(result)
        response._prompt_json = {"messages": messages}

        yield content

    def _fetch_completion(self, api_key: str, payload: dict) -> dict:
        """
        Make synchronous fetch call to OpenAI API.

        Uses pyodide's event loop to make async fetch call appear synchronous.
        """
        import asyncio
        from pyodide.ffi import to_js
        from js import fetch, Headers, JSON

        async def _async_fetch():
            # Build headers
            headers = Headers.new()
            headers.append("Content-Type", "application/json")
            headers.append("Authorization", f"Bearer {api_key}")

            # Convert payload to JS object
            payload_js = to_js(payload, dict_converter=Object.fromEntries)

            # Make fetch request
            options = {
                "method": "POST",
                "headers": headers,
                "body": JSON.stringify(payload_js)
            }
            options_js = to_js(options, dict_converter=Object.fromEntries)

            fetch_response = await fetch(
                "https://api.openai.com/v1/chat/completions",
                options_js
            )

            # Check status
            if not fetch_response.ok:
                error_text = await fetch_response.text()
                raise Exception(
                    f"OpenAI API error (status {fetch_response.status}): {error_text}"
                )

            # Parse JSON response
            result_js = await fetch_response.json()

            # Convert back to Python dict
            result = result_js.to_py()
            return result

        # Run async function in current event loop
        loop = asyncio.get_event_loop()
        return loop.run_until_complete(_async_fetch())
