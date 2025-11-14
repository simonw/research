# Practical Implementation Guide: Custom zkTLS Platform

This guide provides step-by-step instructions for implementing the recommended hybrid zkTLS approach.

---

## Phase 1: Quick Start with Reclaim (Week 1-4)

### Objective
Get WebView scraping working with cryptographic proofs in your Flutter app.

---

### Step 1.1: Deploy Self-Hosted Reclaim Attestor

**Prerequisites:**
- Docker and docker-compose installed
- Linux server (Ubuntu 22.04+ recommended)
- Domain name with SSL certificate
- 4GB RAM, 2 CPU cores minimum

**Deployment:**

```bash
# Clone attestor-core repository
git clone https://github.com/reclaimprotocol/attestor-core.git
cd attestor-core

# Copy environment template
cp .env.example .env

# Edit .env with your configuration
nano .env
```

**Configuration (.env):**
```bash
# Server Configuration
PORT=8000
HOST=0.0.0.0
NODE_ENV=production

# Database (if required)
DATABASE_URL=postgresql://user:pass@localhost:5432/attestor

# Attestor Keys (generate new keypair)
ATTESTOR_PRIVATE_KEY=<generate-new-key>
ATTESTOR_PUBLIC_KEY=<corresponding-public-key>

# Domain
DOMAIN=attestor.yourdomain.com
```

**Generate Attestor Keypair:**
```bash
# Using OpenSSL
openssl ecparam -name secp256k1 -genkey -noout -out attestor-private.pem
openssl ec -in attestor-private.pem -pubout -out attestor-public.pem

# Convert to hex format for .env
# (Implementation-specific, check attestor-core docs)
```

**Start Services:**
```bash
# Build and run with docker-compose
docker-compose up -d

# Check logs
docker-compose logs -f attestor

# Verify it's running
curl https://attestor.yourdomain.com/health
```

**Expected Response:**
```json
{
  "status": "healthy",
  "version": "1.0.0",
  "publicKey": "0x..."
}
```

---

### Step 1.2: Integrate Reclaim SDK in Flutter App

**Add Dependency:**
```yaml
# pubspec.yaml
dependencies:
  flutter:
    sdk: flutter
  reclaim_sdk: ^1.0.0  # Check latest version on pub.dev
  webview_flutter: ^4.0.0
```

**Install:**
```bash
flutter pub get
```

**Basic Integration:**
```dart
// lib/services/reclaim_service.dart
import 'package:reclaim_sdk/reclaim_sdk.dart';

class ReclaimService {
  late ReclaimProofRequest proofRequest;

  // Initialize with your self-hosted attestor
  Future<void> initialize() async {
    await Reclaim.init(
      appId: 'your-app-id',
      appSecret: 'your-app-secret',
      attestorUrl: 'https://attestor.yourdomain.com'  // Your server
    );
  }

  // Create proof request for LinkedIn
  Future<ReclaimProofRequest> createLinkedInProofRequest() async {
    proofRequest = await ReclaimProofRequest.init(
      'your-app-id',
      'your-app-secret',
      'linkedin-profile-provider',  // Pre-built provider
    );

    // Configure what data to extract
    proofRequest.setAppCallbackUrl('myapp://callback');

    return proofRequest;
  }

  // Start proof generation
  Future<String> startProofGeneration() async {
    final url = await proofRequest.getRequestUrl();
    return url;
  }

  // Handle callback and get proof
  Future<Proof> getProof(String callbackData) async {
    final proofs = await proofRequest.getProofs(callbackData);
    return proofs.first;
  }

  // Verify proof
  Future<bool> verifyProof(Proof proof) async {
    return await Reclaim.verifyProof(proof);
  }
}
```

**UI Implementation:**
```dart
// lib/screens/linkedin_scraper.dart
import 'package:flutter/material.dart';
import 'package:webview_flutter/webview_flutter.dart';
import 'package:reclaim_sdk/reclaim_sdk.dart';
import '../services/reclaim_service.dart';

class LinkedInScraperScreen extends StatefulWidget {
  @override
  _LinkedInScraperScreenState createState() => _LinkedInScraperScreenState();
}

class _LinkedInScraperScreenState extends State<LinkedInScraperScreen> {
  final ReclaimService _reclaimService = ReclaimService();
  late WebViewController _webViewController;
  String? _proofData;
  bool _isGeneratingProof = false;

  @override
  void initState() {
    super.initState();
    _initializeReclaim();
  }

  Future<void> _initializeReclaim() async {
    await _reclaimService.initialize();
  }

  Future<void> _generateProof() async {
    setState(() => _isGeneratingProof = true);

    try {
      // Create proof request
      final proofRequest = await _reclaimService.createLinkedInProofRequest();

      // Get URL for webview
      final url = await _reclaimService.startProofGeneration();

      // Load in webview
      await _webViewController.loadRequest(Uri.parse(url));

      // Proof will be generated automatically by Reclaim's webview flow
      // Handle via callback

    } catch (e) {
      print('Error generating proof: $e');
    } finally {
      setState(() => _isGeneratingProof = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text('LinkedIn Data Scraper')),
      body: Column(
        children: [
          Expanded(
            child: WebViewWidget(
              controller: WebViewController()
                ..setJavaScriptMode(JavaScriptMode.unrestricted)
                ..setNavigationDelegate(
                  NavigationDelegate(
                    onPageFinished: (url) {
                      // Handle page load
                    },
                    onNavigationRequest: (request) {
                      // Handle Reclaim callback
                      if (request.url.startsWith('myapp://callback')) {
                        _handleReclaimCallback(request.url);
                        return NavigationDecision.prevent;
                      }
                      return NavigationDecision.navigate;
                    },
                  ),
                ),
            ),
          ),
          if (_isGeneratingProof)
            LinearProgressIndicator(),
          Padding(
            padding: EdgeInsets.all(16),
            child: ElevatedButton(
              onPressed: _isGeneratingProof ? null : _generateProof,
              child: Text('Generate Proof'),
            ),
          ),
          if (_proofData != null)
            Padding(
              padding: EdgeInsets.all(16),
              child: Text('Proof: $_proofData'),
            ),
        ],
      ),
    );
  }

  Future<void> _handleReclaimCallback(String url) async {
    try {
      // Extract callback data from URL
      final uri = Uri.parse(url);
      final callbackData = uri.queryParameters['data'];

      if (callbackData != null) {
        // Get proof
        final proof = await _reclaimService.getProof(callbackData);

        // Verify proof
        final isValid = await _reclaimService.verifyProof(proof);

        if (isValid) {
          setState(() {
            _proofData = proof.claimData;
          });

          // Send to your backend
          await _sendToBackend(proof);
        }
      }
    } catch (e) {
      print('Error handling callback: $e');
    }
  }

  Future<void> _sendToBackend(Proof proof) async {
    // Send proof to your backend for storage/verification
    // Implementation depends on your backend
  }
}
```

---

### Step 1.3: Create Custom Provider for LinkedIn

If the pre-built LinkedIn provider doesn't meet your needs:

**Provider Schema (JSON):**
```json
{
  "name": "linkedin-custom",
  "version": "1.0",
  "url": "https://www.linkedin.com/in/{username}/",
  "loginUrl": "https://www.linkedin.com/login",
  "selectors": {
    "name": {
      "selector": "h1.text-heading-xlarge",
      "type": "text"
    },
    "headline": {
      "selector": "div.text-body-medium",
      "type": "text"
    },
    "experience": {
      "selector": "#experience-section li",
      "type": "list",
      "fields": {
        "title": ".pv-entity__summary-info h3",
        "company": ".pv-entity__secondary-title",
        "dates": ".pv-entity__date-range span:nth-child(2)"
      }
    }
  },
  "proofParams": {
    "includes": ["name", "headline", "experience"],
    "redact": ["profileImage", "email"]
  }
}
```

**Register Provider:**
```bash
# Using Reclaim CLI or API
curl -X POST https://attestor.yourdomain.com/providers \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d @linkedin-custom-provider.json
```

---

### Step 1.4: Backend Verification

**Node.js Example:**
```javascript
// backend/services/proof-verifier.js
const { Reclaim } = require('@reclaimprotocol/js-sdk');

class ProofVerifier {
  constructor() {
    this.reclaim = new Reclaim({
      attestorUrl: 'https://attestor.yourdomain.com'
    });
  }

  async verifyProof(proofData) {
    try {
      // Verify signature
      const isValid = await this.reclaim.verifyProof(proofData);

      if (!isValid) {
        throw new Error('Invalid proof signature');
      }

      // Extract and validate claims
      const claims = this.extractClaims(proofData);

      // Additional validation logic
      this.validateClaims(claims);

      return {
        valid: true,
        claims: claims,
        timestamp: proofData.timestamp,
        attestor: proofData.attestorPublicKey
      };

    } catch (error) {
      console.error('Proof verification failed:', error);
      return {
        valid: false,
        error: error.message
      };
    }
  }

  extractClaims(proofData) {
    // Parse claim data from proof
    return JSON.parse(proofData.claimData);
  }

  validateClaims(claims) {
    // Business logic validation
    if (!claims.name || claims.name.length < 2) {
      throw new Error('Invalid name in claims');
    }
    // More validation...
  }
}

module.exports = ProofVerifier;
```

**Express API Endpoint:**
```javascript
// backend/routes/proofs.js
const express = require('express');
const ProofVerifier = require('../services/proof-verifier');

const router = express.Router();
const verifier = new ProofVerifier();

router.post('/verify', async (req, res) => {
  try {
    const { proof } = req.body;

    // Verify proof
    const result = await verifier.verifyProof(proof);

    if (result.valid) {
      // Store in database
      await db.proofs.create({
        userId: req.user.id,
        claims: result.claims,
        timestamp: result.timestamp,
        attestor: result.attestor,
        proofData: proof
      });

      res.json({
        success: true,
        claims: result.claims
      });
    } else {
      res.status(400).json({
        success: false,
        error: result.error
      });
    }

  } catch (error) {
    res.status(500).json({
      success: false,
      error: error.message
    });
  }
});

module.exports = router;
```

---

## Phase 2: TLSNotary Foundation (Week 5-12)

### Objective
Deploy self-hosted TLSNotary infrastructure for long-term customization.

---

### Step 2.1: Deploy TLSNotary Notary Server

**Prerequisites:**
- Linux server (Ubuntu 22.04+)
- Rust toolchain installed
- 8GB RAM, 4 CPU cores
- Domain with SSL certificate

**Installation:**
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Clone TLSNotary repository
git clone https://github.com/tlsnotary/tlsn.git
cd tlsn

# Build notary server
cd notary-server
cargo build --release

# Binary will be at: target/release/notary-server
```

**Configuration:**
```bash
# Create config file
nano config.yaml
```

**config.yaml:**
```yaml
server:
  host: 0.0.0.0
  port: 7047

tls:
  enabled: true
  cert_path: /etc/letsencrypt/live/notary.yourdomain.com/fullchain.pem
  key_path: /etc/letsencrypt/live/notary.yourdomain.com/privkey.pem

notary:
  # Maximum number of concurrent sessions
  max_sessions: 100

  # Session timeout (seconds)
  session_timeout: 300

  # Maximum transcript size (bytes)
  max_transcript_size: 16384

  # Logging
  log_level: info

# Optional: TEE/SGX configuration
tee:
  enabled: false
  # sgx_mode: hw
```

**Run as systemd service:**
```bash
# Create systemd service file
sudo nano /etc/systemd/system/tlsnotary.service
```

**/etc/systemd/system/tlsnotary.service:**
```ini
[Unit]
Description=TLSNotary Notary Server
After=network.target

[Service]
Type=simple
User=tlsnotary
WorkingDirectory=/opt/tlsnotary
ExecStart=/opt/tlsnotary/notary-server --config /opt/tlsnotary/config.yaml
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

**Start service:**
```bash
# Create user and directories
sudo useradd -r -s /bin/false tlsnotary
sudo mkdir -p /opt/tlsnotary
sudo cp target/release/notary-server /opt/tlsnotary/
sudo cp config.yaml /opt/tlsnotary/
sudo chown -R tlsnotary:tlsnotary /opt/tlsnotary

# Enable and start
sudo systemctl enable tlsnotary
sudo systemctl start tlsnotary

# Check status
sudo systemctl status tlsnotary

# View logs
sudo journalctl -u tlsnotary -f
```

**Test notary:**
```bash
# Health check
curl https://notary.yourdomain.com:7047/health

# Expected response:
# {"status":"ok","version":"0.1.0"}
```

---

### Step 2.2: Deploy Multiple Notaries (Redundancy)

For production, deploy 3-5 notary servers in different regions:

**Geographic Distribution:**
- Notary 1: US East (AWS us-east-1)
- Notary 2: US West (AWS us-west-2)
- Notary 3: Europe (AWS eu-west-1)
- Notary 4: Asia (AWS ap-southeast-1)
- Notary 5: Backup (different cloud provider)

**Load Balancer:**
```bash
# Using Nginx as load balancer
sudo nano /etc/nginx/conf.d/tlsnotary-lb.conf
```

**/etc/nginx/conf.d/tlsnotary-lb.conf:**
```nginx
upstream tlsnotary_notaries {
    least_conn;
    server notary1.yourdomain.com:7047 max_fails=3 fail_timeout=30s;
    server notary2.yourdomain.com:7047 max_fails=3 fail_timeout=30s;
    server notary3.yourdomain.com:7047 max_fails=3 fail_timeout=30s;
    server notary4.yourdomain.com:7047 max_fails=3 fail_timeout=30s;
    server notary5.yourdomain.com:7047 max_fails=3 fail_timeout=30s backup;
}

server {
    listen 443 ssl http2;
    server_name notary.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/notary.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/notary.yourdomain.com/privkey.pem;

    location / {
        proxy_pass https://tlsnotary_notaries;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;

        # Timeouts for long MPC sessions
        proxy_connect_timeout 300s;
        proxy_send_timeout 300s;
        proxy_read_timeout 300s;
    }
}
```

---

### Step 2.3: Integrate TLSNotary in Flutter

Since TLSNotary doesn't have an official Flutter SDK, you'll need to build a custom integration:

**Approach 1: WASM via WebView**
```dart
// TLSNotary compiles to WASM, can run in WebView
// This is complex but possible
```

**Approach 2: Native Bridge to Rust**
```dart
// Use Flutter's FFI to call TLSNotary Rust library
// Requires platform-specific builds
```

**Approach 3: Proxy Server** (Recommended for now)
```dart
// Build a proxy server that handles TLSNotary protocol
// Flutter app communicates with proxy via REST API
```

**Proxy Server (Node.js/TypeScript):**
```typescript
// tlsnotary-proxy/src/server.ts
import express from 'express';
import { TLSNotaryClient } from '@tlsnotary/client'; // Hypothetical

const app = express();
app.use(express.json());

app.post('/api/generate-proof', async (req, res) => {
  try {
    const { url, headers, body, notaryUrl } = req.body;

    // Initialize TLSNotary client
    const client = new TLSNotaryClient({
      notaryUrl: notaryUrl || 'https://notary.yourdomain.com:7047'
    });

    // Perform MPC-TLS session
    const session = await client.createSession(url);

    // Send HTTP request through TLS session
    const response = await session.sendRequest({
      method: 'GET',
      headers: headers,
      body: body
    });

    // Generate proof
    const proof = await session.generateProof({
      selectors: req.body.selectors,  // What to prove
      redactions: req.body.redactions  // What to redact
    });

    res.json({
      success: true,
      proof: proof.serialize(),
      data: proof.claimedData
    });

  } catch (error) {
    res.status(500).json({
      success: false,
      error: error.message
    });
  }
});

app.listen(3000, () => {
  console.log('TLSNotary proxy server listening on port 3000');
});
```

**Flutter Integration:**
```dart
// lib/services/tlsnotary_service.dart
import 'package:http/http.dart' as http;
import 'dart:convert';

class TLSNotaryService {
  final String proxyUrl;

  TLSNotaryService({
    required this.proxyUrl,
  });

  Future<TLSNotaryProof> generateProof({
    required String targetUrl,
    required Map<String, String> headers,
    required List<String> selectors,
    required List<String> redactions,
  }) async {
    final response = await http.post(
      Uri.parse('$proxyUrl/api/generate-proof'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({
        'url': targetUrl,
        'headers': headers,
        'selectors': selectors,
        'redactions': redactions,
        'notaryUrl': 'https://notary.yourdomain.com:7047',
      }),
    );

    if (response.statusCode == 200) {
      final data = jsonDecode(response.body);
      return TLSNotaryProof.fromJson(data['proof']);
    } else {
      throw Exception('Failed to generate proof: ${response.body}');
    }
  }

  Future<bool> verifyProof(TLSNotaryProof proof) async {
    // Implement proof verification
    // This can be done locally or via API
    return true; // Simplified
  }
}

class TLSNotaryProof {
  final String data;
  final String signature;
  final String notaryPublicKey;

  TLSNotaryProof({
    required this.data,
    required this.signature,
    required this.notaryPublicKey,
  });

  factory TLSNotaryProof.fromJson(Map<String, dynamic> json) {
    return TLSNotaryProof(
      data: json['data'],
      signature: json['signature'],
      notaryPublicKey: json['notaryPublicKey'],
    );
  }
}
```

---

## Phase 3: Native Mobile App Traffic (Week 13-20)

### Objective
Capture traffic from native iOS/Android apps and generate proofs.

**Note:** This builds on your previous research documented in the `traffic-capture-platform` folder.

---

### Step 3.1: Implement VPN Layer

**iOS (Swift):**
```swift
// VPNManager.swift
import NetworkExtension

class VPNManager {
    static let shared = VPNManager()

    func setupVPN() {
        let manager = NEVPNManager.shared()
        manager.loadFromPreferences { error in
            if let error = error {
                print("Error loading VPN: \\(error)")
                return
            }

            let proto = NEVPNProtocolIKEv2()
            proto.serverAddress = "vpn.yourdomain.com"
            proto.username = "user"
            proto.passwordReference = /* keychain reference */
            proto.authenticationMethod = .sharedSecret

            manager.protocolConfiguration = proto
            manager.localizedDescription = "Data Attestation VPN"
            manager.isEnabled = true

            manager.saveToPreferences { error in
                if let error = error {
                    print("Error saving VPN: \\(error)")
                } else {
                    self.startVPN()
                }
            }
        }
    }

    func startVPN() {
        do {
            try NEVPNManager.shared().connection.startVPNTunnel()
        } catch {
            print("Error starting VPN: \\(error)")
        }
    }
}
```

**Android (Kotlin):**
```kotlin
// VpnService.kt
import android.net.VpnService
import android.os.ParcelFileDescriptor

class AttestationVpnService : VpnService() {
    private var vpnInterface: ParcelFileDescriptor? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val builder = Builder()
        builder.setSession("Data Attestation VPN")
        builder.addAddress("10.0.0.2", 32)
        builder.addRoute("0.0.0.0", 0)
        builder.addDnsServer("8.8.8.8")

        vpnInterface = builder.establish()

        // Start packet forwarding thread
        Thread {
            forwardPackets()
        }.start()

        return START_STICKY
    }

    private fun forwardPackets() {
        val inputStream = FileInputStream(vpnInterface!!.fileDescriptor)
        val outputStream = FileOutputStream(vpnInterface!!.fileDescriptor)

        // Read packets, intercept TLS, forward to attestor
        // Implementation details in your prior research
    }
}
```

---

### Step 3.2: TLS Interception for Attestation

**Proxy Server (for TLS interception):**
```python
# tls-interceptor/proxy.py
import asyncio
from mitmproxy import http, ctx
from tlsnotary_client import TLSNotaryClient

class AttestationProxy:
    def __init__(self):
        self.tlsnotary_client = TLSNotaryClient(
            notary_url="https://notary.yourdomain.com:7047"
        )

    async def request(self, flow: http.HTTPFlow) -> None:
        """Intercept HTTP request"""
        # Log request for later attestation
        ctx.log.info(f"Request to {flow.request.url}")

    async def response(self, flow: http.HTTPFlow) -> None:
        """Intercept HTTP response and generate proof"""
        # Check if this response should be attested
        if self.should_attest(flow):
            try:
                # Generate TLSNotary proof
                proof = await self.tlsnotary_client.generate_proof(
                    url=flow.request.url,
                    request_headers=dict(flow.request.headers),
                    response_body=flow.response.content,
                )

                # Attach proof to response headers
                flow.response.headers["X-Attestation-Proof"] = proof.serialize()

                ctx.log.info(f"Generated proof for {flow.request.url}")

            except Exception as e:
                ctx.log.error(f"Failed to generate proof: {e}")

    def should_attest(self, flow: http.HTTPFlow) -> bool:
        """Determine if this request should be attested"""
        # Example: Only attest API calls
        return '/api/' in flow.request.path

addons = [AttestationProxy()]
```

**Run Proxy:**
```bash
# Install mitmproxy
pip install mitmproxy

# Run with custom script
mitmproxy -s tls-interceptor/proxy.py --listen-port 8080
```

---

## Phase 4: IoT and Streaming Extensions (Week 21-32)

### Objective
Extend TLSNotary to support WebSocket, MQTT, and streaming data.

---

### Step 4.1: WebSocket-over-TLS Extension

**Extended Notary Server (Rust):**
```rust
// tlsn-websocket/src/websocket_session.rs
use tokio_tungstenite::WebSocketStream;
use tlsnotary_core::Session;

pub struct WebSocketSession {
    session: Session,
    message_buffer: Vec<Message>,
    merkle_tree: MerkleTree,
    batch_interval: Duration,
}

impl WebSocketSession {
    pub async fn new(notary_url: &str) -> Result<Self> {
        let session = Session::new(notary_url).await?;

        Ok(Self {
            session,
            message_buffer: Vec::new(),
            merkMalpractice Insurance

Published on January 2017 | Categories: Documents | Downloads: 16 | Comments: 0 | Views: 172
of 127
Download PDF   Embed   Report

Comments

Content

JOURNAL OF EMPIRICAL LEGAL STUDIES

Volume 3, Issue 2, 183–217, July 2006

Medical Malpractice and the Vanishing Jury Trial: The Allure of Secret Settlement in the Era of Managed Care
Kenneth W. Abbott, John L. Hudson, and Daniel A. Rodriguez*

The well-documented reduction in tort trials in recent years seems especially surprising when it comes to medical malpractice cases. Malpractice issues are unusually controversial, and the conventional wisdom suggests that fear of jury bias against providers should induce managed care organizations (MCOs)—which increasingly control health care decisions—to demand trials. In this article, we test the conventional wisdom about jury bias with claims processing data for a large MCO. We also consider the possibility that MCOs and other repeat players (RPs), who can view lawsuits from a large perspective, should more often settle cases than isolated litigants (often referred to as "oneshots"). We find that MCO juries are not unusually biased against the MCO, and they typically award lower damages than the MCO's settlement payments—but that claims are settled at a rate nearly as high as in other fields. We suggest that the structure and dynamics of settlement in health care malpractice explain why RPs like MCOs are able to settle most malpractice claims, even though plaintiffs are likely to receive less in settlement than they would have been awarded at trial. Our findings challenge the conventional wisdom regarding RP advantages in litigation and clarify the

*Address correspondence to Daniel A. Rodriguez, Sandra Day O'Connor College of Law, Arizona State University, PO Box 877906, Tempe, AZ 85287-7906. Abbott is Regents Professor at the Sandra Day O'Connor College of Law, Arizona State University; Hudon is Professor of Economics at IUPUI School of Liberal Arts and, since 1995, has served as Chief Economist with the American Association of Health Plans; Rodriguez is Professor and Director of the Center for the Study of Law, Science, and Technology at the Sandra Day O'Connor College of Law. We are grateful to Peter Sander, who in his role at the American Association of Health Plans helped initiate this project, and to our colleagues at the AAHP and at ASU. We are also grateful for fine research assistance from Tom Newman and Thom Walsh at ASU. Last, but certainly not least, we appreciate the patience and good judgment of the management and staff of the large MCO that made available to us the data on which this study is principally based. © 2006, Copyright the Authors Journal compilation © 2006, Cornell Law School and Wiley Periodicals, Inc.

183

184

Abbott et al.

dynamics of settlement in health care malpractice litigation, which are increasingly dominated by managed care organizations.

I. Introduction Despite fears of a litigation explosion, the past two decades have witnessed a steady decline in the frequency with which civil cases that are filed in both state and federal courts are terminated by a jury trial. This trend is well documented in studies by Galanter (2002, 2004) and others; indeed, while there are important variations based on the type of case (e.g., products liability versus contract), the overall reduction of civil jury trials in the United States seems beyond doubt. Malpractice litigation, a principal issue in contemporary tort reform debates, would seem an unlikely area for such a reduction: malpractice issues are unusually controversial; the rhetoric of tort reform emphasizes the fear of massive jury awards; and the conventional wisdom suggests that fear of jury bias against providers should induce managed care organizations (MCOs), which increasingly control health care decisions, to defend their cases, including going to trial if necessary. This article suggests an alternative view of the decline in jury trials for malpractice litigation; it is designed to test the conventional wisdom with empirical data on claims processing by a large MCO. Our claims data supports earlier qualitative research showing that there is little empirical support for the supposition that MCO juries are biased against the MCO. Moreover, the data also reveals that juries typically award lower damages than the MCO's settlement payments. Nonetheless, by a combination of factors rooted in the structure of contemporary health care and in the nature of settlement, the overwhelming number of claims are settled before trial. This article is designed to illuminate the relationship between this structure—and the particular role of dominant health care providers as "repeat players," to use Marc Galanter's terminology, in an era of managed care—and the settlement dynamics of managed care litigation. Our empirical analysis of a significant data set brings forth evidence that is both consistent with the trend line first reported by Galanter, yet also illuminates a phenomenon that is by no means inevitable, especially given the distinctive qualities of MCO malpractice litigation. Our analysis has two primary purposes. First, we seek to illuminate the structure and dynamics of managed care malpractice litigation. An understanding of contemporary malpractice litigation in general