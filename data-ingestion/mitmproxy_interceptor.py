"""
mitmproxy Custom Data Interceptor - Proof of Concept

This POC demonstrates HTTPS interception with mitmproxy to extract
data from API requests and responses.

IMPORTANT LIMITATIONS:
- Requires CA certificate installation (poor UX)
- No cryptographic provenance guarantees
- Blocked by certificate pinning
- User must trust the proxy completely

Use Case: Development/testing only, NOT production

Setup:
1. Install mitmproxy: pip install mitmproxy
2. Run: mitmproxy -s mitmproxy_interceptor.py
3. Configure device proxy: localhost:8080
4. Install cert: visit mitm.it in device browser

Usage:
  mitmproxy -s mitmproxy_interceptor.py
  OR
  mitmdump -s mitmproxy_interceptor.py --quiet
"""

import json
import re
from mitmproxy import http, ctx
from datetime import datetime
from typing import Dict, Any


class DataInterceptor:
    """
    Custom mitmproxy addon for intercepting and extracting data
    from specific API endpoints.
    """

    def __init__(self):
        self.intercepted_data = []
        self.patterns = {
            # Define API endpoints to intercept
            'linkedin_profile': {
                'pattern': r'api\.linkedin\.com.*\/profile',
                'extractor': self.extract_linkedin_profile
            },
            'twitter_api': {
                'pattern': r'api\.twitter\.com.*\/users',
                'extractor': self.extract_twitter_data
            },
            'generic_api': {
                'pattern': r'api\.',  # Catch-all for any API
                'extractor': self.extract_generic_json
            }
        }

    def request(self, flow: http.HTTPFlow) -> None:
        """
        Intercept and log requests.
        Can modify requests here (e.g., inject auth tokens).
        """
        request = flow.request

        # Log request details
        ctx.log.info(f"Request: {request.method} {request.pretty_url}")

        # Example: Inject custom headers
        # request.headers["X-Custom-Header"] = "custom-value"

        # Example: Modify request body
        # if request.method == "POST":
        #     request.text = self.modify_request_body(request.text)

    def response(self, flow: http.HTTPFlow) -> None:
        """
        Intercept responses and extract data based on patterns.
        """
        request = flow.request
        response = flow.response

        # Check if this is a URL we want to intercept
        for source_name, config in self.patterns.items():
            if re.search(config['pattern'], request.pretty_url):
                ctx.log.info(f"✓ Matched pattern: {source_name}")

                # Extract data using the configured extractor
                try:
                    extracted_data = config['extractor'](flow)
                    if extracted_data:
                        self.save_extracted_data(
                            source_name,
                            request.pretty_url,
                            extracted_data
                        )
                except Exception as e:
                    ctx.log.error(f"Error extracting data: {e}")

    def extract_linkedin_profile(self, flow: http.HTTPFlow) -> Dict[str, Any]:
        """
        Extract LinkedIn profile data from API response.
        """
        try:
            response_text = flow.response.text
            data = json.loads(response_text)

            # Extract relevant fields
            extracted = {
                'name': data.get('firstName', '') + ' ' + data.get('lastName', ''),
                'headline': data.get('headline', ''),
                'education': data.get('education', []),
                'experience': data.get('experience', []),
                'skills': data.get('skills', [])
            }

            ctx.log.info(f"Extracted LinkedIn profile: {extracted.get('name')}")
            return extracted

        except json.JSONDecodeError:
            ctx.log.error("Failed to parse LinkedIn response as JSON")
            return {}

    def extract_twitter_data(self, flow: http.HTTPFlow) -> Dict[str, Any]:
        """
        Extract Twitter/X user data from API response.
        """
        try:
            response_text = flow.response.text
            data = json.loads(response_text)

            extracted = {
                'username': data.get('screen_name', ''),
                'followers': data.get('followers_count', 0),
                'following': data.get('friends_count', 0),
                'tweets': data.get('statuses_count', 0),
                'verified': data.get('verified', False)
            }

            return extracted

        except json.JSONDecodeError:
            return {}

    def extract_generic_json(self, flow: http.HTTPFlow) -> Dict[str, Any]:
        """
        Generic JSON extractor for unknown APIs.
        """
        try:
            response_text = flow.response.text
            data = json.loads(response_text)

            # Return the full JSON response
            return {
                'raw_response': data,
                'response_size': len(response_text)
            }

        except json.JSONDecodeError:
            # Not JSON, return text snippet
            return {
                'content_type': flow.response.headers.get('content-type', 'unknown'),
                'text_snippet': flow.response.text[:200]
            }

    def save_extracted_data(
        self,
        source_name: str,
        url: str,
        data: Dict[str, Any]
    ) -> None:
        """
        Save extracted data to storage.
        """
        entry = {
            'timestamp': datetime.now().isoformat(),
            'source': source_name,
            'url': url,
            'data': data,
            'provenance': None,  # NOTE: No cryptographic proof!
            'verified': False    # NOTE: Cannot verify authenticity!
        }

        self.intercepted_data.append(entry)

        # Log to file
        with open('intercepted_data.json', 'a') as f:
            f.write(json.dumps(entry) + '\n')

        ctx.log.info(f"✓ Saved data from {source_name}")

    def modify_request_body(self, body: str) -> str:
        """
        Example: Modify request body before sending.
        """
        try:
            data = json.loads(body)
            # Inject custom parameters
            data['custom_param'] = 'injected_value'
            return json.dumps(data)
        except:
            return body


class JavaScriptInjector:
    """
    Inject custom JavaScript into HTML responses
    for client-side data extraction.
    """

    def __init__(self):
        self.injection_script = """
        <script>
        (function() {
            // Custom data extraction script
            console.log('[MITM] Data extraction script loaded');

            // Example: Extract data from page
            function extractPageData() {
                const data = {
                    title: document.title,
                    url: window.location.href,
                    // Add custom extraction logic here
                };

                // Send to proxy server
                fetch('http://localhost:8080/extracted', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify(data)
                });
            }

            // Run extraction after page load
            if (document.readyState === 'complete') {
                extractPageData();
            } else {
                window.addEventListener('load', extractPageData);
            }
        })();
        </script>
        """

    def response(self, flow: http.HTTPFlow) -> None:
        """
        Inject JavaScript into HTML responses.
        """
        if flow.response and 'text/html' in flow.response.headers.get('content-type', ''):
            content = flow.response.text

            # Inject script before </body>
            if '</body>' in content:
                content = content.replace('</body>', self.injection_script + '</body>')
                flow.response.text = content
                ctx.log.info(f"✓ Injected JS into {flow.request.pretty_url}")


class StreamingDataInterceptor:
    """
    Handle streaming data sources (WebSocket, SSE, etc.)
    """

    def websocket_message(self, flow: http.HTTPFlow) -> None:
        """
        Intercept WebSocket messages.
        """
        message = flow.websocket.messages[-1]

        ctx.log.info(f"WebSocket [{message.from_client and 'Client' or 'Server'}]: "
                     f"{message.content[:100]}...")

        # Parse and extract data from WebSocket messages
        try:
            data = json.loads(message.content)
            ctx.log.info(f"WebSocket data: {data}")
            # Process streaming data here
        except:
            pass


# Register addons
addons = [
    DataInterceptor(),
    # JavaScriptInjector(),  # Uncomment to enable JS injection
    # StreamingDataInterceptor(),  # Uncomment for WebSocket support
]


def main():
    """
    Standalone execution (for testing).
    """
    print("""
    ═══════════════════════════════════════════════════════
    mitmproxy Data Interceptor POC
    ═══════════════════════════════════════════════════════

    To run:
    1. mitmproxy -s mitmproxy_interceptor.py
    2. Configure device proxy: localhost:8080
    3. Install certificate: visit mitm.it

    To run in quiet mode (no UI):
    mitmdump -s mitmproxy_interceptor.py --quiet

    Intercepted data will be saved to: intercepted_data.json

    LIMITATIONS:
    ⚠ Requires CA certificate installation (poor UX)
    ⚠ No cryptographic provenance guarantees
    ⚠ Blocked by certificate pinning (Twitter, Instagram, etc.)
    ⚠ User must trust proxy completely
    ⚠ Not suitable for production use

    RECOMMENDED ALTERNATIVE: Use zkTLS (Reclaim/TLSNotary)
    ═══════════════════════════════════════════════════════
    """)


if __name__ == '__main__':
    main()
