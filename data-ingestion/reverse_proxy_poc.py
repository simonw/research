"""
Reverse Proxy for Data Ingestion - Proof of Concept

This POC demonstrates a reverse proxy approach for intercepting
and extracting data from API requests without modifying client apps.

Architecture:
Client App -> Reverse Proxy (this) -> Target API Server
                   |
                   v
            Data Extraction & Storage

Advantages:
- No client-side modifications needed
- Can inject custom logic transparently
- Works with mobile apps (via proxy settings)

Limitations:
- Still requires proxy configuration on client
- Certificate trust issues with HTTPS
- No cryptographic provenance guarantees

Dependencies:
  pip install flask requests
"""

from flask import Flask, request, Response
import requests
from typing import Dict, Any, Optional
import json
import logging
from datetime import datetime

app = Flask(__name__)
logging.basicConfig(level=logging.INFO)


class ReverseProxyExtractor:
    """
    Reverse proxy with data extraction capabilities.
    """

    def __init__(self):
        self.extractors = {
            'linkedin.com': self.extract_linkedin_data,
            'twitter.com': self.extract_twitter_data,
            'api.github.com': self.extract_github_data,
        }
        self.intercepted_data = []

    def proxy_request(
        self,
        method: str,
        target_url: str,
        headers: Dict[str, str],
        data: Optional[bytes] = None
    ) -> Response:
        """
        Forward request to target server and return response.
        """
        try:
            # Forward the request to the actual target
            response = requests.request(
                method=method,
                url=target_url,
                headers=headers,
                data=data,
                allow_redirects=False,
                verify=True
            )

            # Create Flask response
            proxy_response = Response(
                response.content,
                status=response.status_code,
                headers=dict(response.headers)
            )

            return proxy_response

        except Exception as e:
            logging.error(f"Proxy error: {e}")
            return Response(
                json.dumps({'error': 'Proxy failed'}),
                status=502,
                mimetype='application/json'
            )

    def extract_data(self, url: str, response_data: bytes) -> Optional[Dict[str, Any]]:
        """
        Extract data based on URL pattern.
        """
        for domain, extractor in self.extractors.items():
            if domain in url:
                try:
                    return extractor(response_data)
                except Exception as e:
                    logging.error(f"Extraction error: {e}")
        return None

    def extract_linkedin_data(self, response_data: bytes) -> Dict[str, Any]:
        """Extract LinkedIn profile data."""
        try:
            data = json.loads(response_data)
            return {
                'type': 'linkedin_profile',
                'name': data.get('firstName', '') + ' ' + data.get('lastName', ''),
                'title': data.get('headline', ''),
                'connections': data.get('numConnections', 0)
            }
        except:
            return {}

    def extract_twitter_data(self, response_data: bytes) -> Dict[str, Any]:
        """Extract Twitter/X data."""
        try:
            data = json.loads(response_data)
            return {
                'type': 'twitter_profile',
                'username': data.get('screen_name', ''),
                'followers': data.get('followers_count', 0)
            }
        except:
            return {}

    def extract_github_data(self, response_data: bytes) -> Dict[str, Any]:
        """Extract GitHub data."""
        try:
            data = json.loads(response_data)
            return {
                'type': 'github_profile',
                'username': data.get('login', ''),
                'repos': data.get('public_repos', 0),
                'followers': data.get('followers', 0)
            }
        except:
            return {}

    def save_extracted_data(self, url: str, data: Dict[str, Any]) -> None:
        """Save extracted data."""
        entry = {
            'timestamp': datetime.now().isoformat(),
            'url': url,
            'data': data,
            'provenance': None,  # No cryptographic proof
            'verified': False    # Cannot verify authenticity
        }
        self.intercepted_data.append(entry)
        logging.info(f"✓ Extracted data from {url}")


# Global proxy instance
proxy = ReverseProxyExtractor()


@app.route('/', defaults={'path': ''}, methods=['GET', 'POST', 'PUT', 'DELETE', 'PATCH'])
@app.route('/<path:path>', methods=['GET', 'POST', 'PUT', 'DELETE', 'PATCH'])
def proxy_handler(path):
    """
    Handle all incoming requests and proxy them to the target.
    """
    # Get the target URL from headers (custom header from client)
    target_host = request.headers.get('X-Target-Host')

    if not target_host:
        return json.dumps({'error': 'Missing X-Target-Host header'}), 400

    # Construct full target URL
    target_url = f"https://{target_host}/{path}"
    if request.query_string:
        target_url += f"?{request.query_string.decode()}"

    logging.info(f"Proxying: {request.method} {target_url}")

    # Forward request
    headers = {k: v for k, v in request.headers if k != 'Host'}
    response = proxy.proxy_request(
        method=request.method,
        target_url=target_url,
        headers=headers,
        data=request.get_data()
    )

    # Extract data if applicable
    extracted = proxy.extract_data(target_url, response.data)
    if extracted:
        proxy.save_extracted_data(target_url, extracted)

    return response


@app.route('/extracted-data', methods=['GET'])
def get_extracted_data():
    """
    Endpoint to retrieve all extracted data.
    """
    return json.dumps(proxy.intercepted_data, indent=2), 200


@app.route('/health', methods=['GET'])
def health_check():
    """Health check endpoint."""
    return json.dumps({
        'status': 'healthy',
        'intercepted_count': len(proxy.intercepted_data)
    }), 200


class ZKTLSIntegratedProxy:
    """
    Conceptual design: Reverse proxy integrated with zkTLS
    for cryptographic provenance guarantees.

    This combines the benefits of:
    - Reverse proxy (transparent interception)
    - zkTLS (cryptographic proof of data authenticity)
    """

    def __init__(self, zktls_provider):
        """
        Initialize with a zkTLS provider (Reclaim, TLSNotary, etc.)
        """
        self.zktls_provider = zktls_provider
        self.proxy = ReverseProxyExtractor()

    async def proxy_with_proof(
        self,
        method: str,
        url: str,
        headers: Dict[str, str],
        data: Optional[bytes] = None
    ) -> Dict[str, Any]:
        """
        Proxy request AND generate zkTLS proof.

        Returns:
        {
            'response': <original response>,
            'proof': <cryptographic proof>,
            'verified': True/False
        }
        """
        # Step 1: Make the request through zkTLS provider
        # (This would use the actual zkTLS SDK)
        result = await self.zktls_provider.request_with_proof(
            method=method,
            url=url,
            headers=headers,
            data=data
        )

        # Step 2: Extract data as usual
        extracted = self.proxy.extract_data(url, result['response'])

        # Step 3: Return response with cryptographic proof
        return {
            'response': result['response'],
            'extracted_data': extracted,
            'proof': result['proof'],  # zkTLS proof
            'verified': result['verified'],
            'timestamp': datetime.now().isoformat()
        }


def main():
    """
    Run the reverse proxy server.
    """
    print("""
    ═══════════════════════════════════════════════════════
    Reverse Proxy Data Extractor POC
    ═══════════════════════════════════════════════════════

    Starting reverse proxy on http://localhost:8081

    Usage:
    1. Configure client to use proxy: localhost:8081
    2. Add X-Target-Host header with target domain
    3. Proxy will intercept and extract data

    Example request:
    curl -X GET http://localhost:8081/users/octocat \\
      -H "X-Target-Host: api.github.com"

    View extracted data:
    curl http://localhost:8081/extracted-data

    LIMITATIONS:
    ⚠ No cryptographic provenance guarantees
    ⚠ Requires proxy configuration on client
    ⚠ Certificate trust issues with HTTPS
    ⚠ Not suitable for production without zkTLS integration

    RECOMMENDED: Integrate with zkTLS for provenance guarantees
    ═══════════════════════════════════════════════════════
    """)

    app.run(host='0.0.0.0', port=8081, debug=False)


if __name__ == '__main__':
    main()
