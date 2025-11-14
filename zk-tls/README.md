# ZK-TLS Landscape Analysis: Replacing Reclaim Protocol for Custom Data Ingestion

**Research Date:** November 14, 2025
**Purpose:** Evaluate zkTLS solutions to replace Reclaim Protocol with a more customizable solution for our data ingestion pipeline

---

## Executive Summary

After comprehensive research into the zkTLS landscape, I recommend a **hybrid approach** leveraging:

1. **TLSNotary (self-hosted)** as the cryptographic foundation for maximum flexibility and customization
2. **Reclaim's attestor-core (self-hosted)** for rapid deployment of WebView-based scraping
3. **Custom protocol extensions** for IoT and streaming data sources (built on TLSNotary's crypto primitives)

**Critical Finding:** No existing zkTLS solution supports IoT protocols (MQTT, WebSocket streaming) or real-time data attestation. This represents both a challenge and an opportunity for custom development.

---

## Table of Contents

1. [Use Case Requirements](#use-case-requirements)
2. [zkTLS Landscape Overview](#zktls-landscape-overview)
3. [Detailed Solution Analysis](#detailed-solution-analysis)
4. [Gap Analysis: IoT and Streaming](#gap-analysis-iot-and-streaming)
5. [Recommendation](#recommendation)
6. [Implementation Roadmap](#implementation-roadmap)
7. [Risk Assessment](#risk-assessment)
8. [Conclusion](#conclusion)

---

## Use Case Requirements

Our custom data ingestion pipeline needs to support:

### 1. **Web Scraping via Flutter WebView**
- Custom JavaScript injection in webviews
- Scraping data from sites like LinkedIn
- Cryptographic proof that data originated from the claimed source
- Mobile-first (iOS/Android)

### 2. **Native Mobile Apps**
- Traffic capture from native iOS/Android applications
- Data provenance verification
- Support for certificate-pinned apps

### 3. **IoT Devices**
- Data verification from IoT sensors and devices
- Support for constrained devices
- Low-latency verification

### 4. **Streaming Sources**
- WebSocket connections
- MQTT message streams
- Real-time data attestation
- Continuous verification (not just one-time proofs)

### 5. **Core Cryptographic Requirement**
Anyone should be able to verify that captured data originated from a specific source with cryptographic certainty, without trusting our infrastructure.

---

## zkTLS Landscape Overview

### What is zkTLS?

Zero-Knowledge Transport Layer Security (zkTLS) combines zero-knowledge proofs with TLS to cryptographically verify that data from an HTTPS connection is authentic while preserving privacy. It enables proving "I received data X from server Y" without revealing sensitive information or requiring cooperation from the server.

### Key Players in 2025

| Solution | Maturity | Architecture | Self-Hosted | Open Source | Focus Area |
|----------|----------|--------------|-------------|-------------|------------|
| **TLSNotary** | Production | MPC-based | ✅ Yes | ✅ Apache2/MIT | General zkTLS foundation |
| **Reclaim Protocol** | Production (1M+ proofs) | Proxy-based | ✅ Yes (attestor-core) | ✅ Partial | Consumer data, Mobile |
| **Opacity Network** | Early (Pilot) | MPC + EigenLayer AVS | ⚠️ Limited | ❌ No | Security-maximalist |
| **Chainlink DECO** | Sandbox | MPC-based | ❌ No | ❌ No | Enterprise/Finance |
| **zkPass** | Beta | Hybrid (MPC + ZK) | ⚠️ Limited | ⚠️ Partial | Consumer identity |
| **ExpandZK** | Early | Hybrid ZK | ❌ No | ❌ No | AI + Web3 |
| **vlayer** | Mainnet (2025) | Built on TLSNotary | ✅ Yes | ✅ Yes | Ethereum integration |

### Two Primary Architectural Approaches

#### **MPC-Based (TLSNotary, DECO)**
- Uses Multi-Party Computation with garbled circuits
- Verifier participates in TLS handshake via MPC
- No single party can forge data
- Higher security guarantees, but more networking overhead
- Typically limited to 2-party (one notary) due to garbled circuits

**Trust Model:** No single attestor needs to be trusted; cryptographic guarantees

#### **Proxy-Based (Reclaim)**
- Attestor acts as proxy between client and server
- Sees encrypted TLS traffic, user controls decryption keys
- Lower latency, faster proof generation
- Can be decentralized via multiple attestors
- Originally proposed in DECO paper, implemented by Reclaim

**Trust Model:** Must trust attestor (or attestor set) not to collude with user

---

## Detailed Solution Analysis

### TLSNotary: The Open Foundation

**Repository:** https://github.com/tlsnotary/tlsn
**License:** Apache 2.0 / MIT (dual)
**Maintainer:** Ethereum Foundation Privacy & Scaling Explorations (PSE)

#### Architecture

TLSNotary operates in three phases:

1. **MPC-TLS Phase:** Client and Notary jointly perform TLS handshake using MPC, splitting session keys so neither party alone can decrypt or forge data.

2. **Selective Disclosure:** User can redact sensitive parts of the TLS transcript, generating zero-knowledge proofs about redacted content.

3. **Verification:** Notary (or any verifier) validates the proof by checking the server's TLS certificate and the cryptographic commitments.

#### Strengths

✅ **Fully open source** with permissive licensing
✅ **Self-hostable** - can run your own notary server
✅ **No single point of trust** - MPC ensures neither party can cheat alone
✅ **Modular architecture** - written in Rust, compiles to WASM
✅ **Strong security model** - peer-reviewed cryptography
✅ **Active development** - backed by Ethereum Foundation
✅ **TEE support** - optional Intel SGX for performance
✅ **Foundation for other projects** - vlayer, Keyring, Usher built on it

#### Weaknesses

❌ **TLS 1.2 only** (TLS 1.3 on roadmap)
❌ **Requires custom integration** - no out-of-box SDKs
❌ **Less mobile-native** - primarily browser/desktop focused
❌ **Limited to one notary** (garbled circuits are 2-party protocols)
❌ **Higher networking overhead** than proxy approaches
⚠️ **No official audit yet** (though based on peer-reviewed DECO research)

#### Self-Hosting

- Public notary server available for testing: `https://notary.pse.dev`
- Can deploy your own notary server
- Docker support
- Versions with `-sgx` suffix run in Intel SGX TEEs for enhanced security
- Full control over notary infrastructure

#### Best For

- Projects requiring maximum cryptographic security
- Custom deployments where you control the notary
- Building custom zkTLS applications from scratch
- Research and experimentation
- Use cases that need provable third-party verification

---

### Reclaim Protocol: The Production-Ready Option

**Repository:** https://github.com/reclaimprotocol/attestor-core
**License:** Varies by component
**Status:** Production (1M+ proofs generated, 25+ client apps)

#### Architecture

Reclaim uses a **proxy-based attestor model**:

1. **Client → Attestor → Server:** User's TLS connection routes through attestor proxy
2. **Key Splitting:** User retains decryption keys; attestor only sees encrypted traffic
3. **Claim Generation:** User creates structured claims about the data received
4. **Attestation:** Attestor validates and cryptographically signs the claim
5. **ZK Proof:** Optional zk-SNARK generated for selective disclosure

#### Strengths

✅ **Production-ready** - 1M+ proofs, 25+ apps, 30+ blockchains
✅ **Excellent mobile support** - Flutter SDK, React Native SDK
✅ **Fast proof generation** - 2-4 seconds on mobile (Groth16 SNARKs)
✅ **889+ data sources** - pre-built providers for major websites
✅ **Easy integration** - "out of the box" guarantee
✅ **Self-hostable** - attestor-core is open source
✅ **Docker support** - docker-compose for easy deployment
✅ **Provider framework** - extensible for custom data sources
✅ **AI-powered templating** - can add new sites in ~30 minutes
✅ **Active ecosystem** - large community, frequent updates

#### Weaknesses

❌ **Single attestor by default** - must trust attestor (or use multiple)
❌ **Less "pure" cryptographically** than MPC approaches
❌ **Proxy model** may not suit all architectures
⚠️ **Decentralization via Eigen AVS** is relatively new
⚠️ **Partial open source** - some components proprietary

#### Self-Hosting

- `attestor-core` repository on GitHub
- TypeScript implementation
- Can run locally or deploy to cloud
- Docker and docker-compose support
- Environment-based configuration (`.env` files)
- Provider schema system for custom data sources
- Eigen AVS for decentralized attestor network (optional)

#### Best For

- Rapid deployment with mobile apps
- Web scraping use cases (LinkedIn, Twitter, etc.)
- Consumer-facing applications
- Projects that need 500+ pre-built data sources
- Teams that want production-ready infrastructure quickly

---

### vlayer: Ethereum-Focused Web Proofs

**Repository:** https://github.com/vlayer-xyz
**Built On:** TLSNotary
**Focus:** Ethereum smart contract integration

#### Architecture

vlayer wraps TLSNotary with additional tooling for Ethereum:

- Uses TLSNotary's MPC-TLS core
- Adds smart contract verifiers
- Web Proofs that can be consumed on-chain
- Developer-friendly SDK for Ethereum dApps

#### Strengths

✅ **Built on TLSNotary** - inherits strong security model
✅ **Mainnet launched** - 480K+ proofs generated in testnet
✅ **Ethereum-native** - designed for smart contract integration
✅ **Web Proofs demo** - live examples available
✅ **Active development** - well-funded project

#### Weaknesses

❌ **Ethereum-specific** - not general-purpose
❌ **Newer platform** - 2025 mainnet launch
⚠️ **Less flexible** than raw TLSNotary for non-Ethereum use cases

#### Best For

- Ethereum dApps requiring web data
- Projects already in Ethereum ecosystem
- Teams wanting pre-built Ethereum integration

---

### Other Solutions: Not Recommended for Your Use Case

#### **Opacity Network**
- **Status:** Early pilots (Nosh, Teleport)
- **Architecture:** MPC + EigenLayer AVS with slashing
- **Why Not:** Still in development, no public self-hosting, complex infrastructure requirements

#### **Chainlink DECO**
- **Status:** Sandbox (enterprise preview)
- **Architecture:** MPC-based (original DECO protocol)
- **Why Not:** Not publicly available, enterprise/finance focused, requires Chainlink infrastructure

#### **zkPass**
- **Status:** Beta (funded $12.5M Series A)
- **Architecture:** Hybrid (MPC + VOLE-based Interactive ZK)
- **Why Not:** Not fully launched, limited self-hosting documentation, SBT-focused model

#### **ExpandZK**
- **Status:** Early development (whitepaper released Jan 2025)
- **Architecture:** Hybrid ZK with advanced proving systems
- **Why Not:** Too early, AI/ML focus not aligned with your use case

---

## Gap Analysis: IoT and Streaming

### Critical Finding

**No existing zkTLS solution supports IoT protocols or streaming data verification.**

Current zkTLS solutions are designed exclusively for:
- HTTPS/TLS 1.2 web traffic
- Request-response patterns
- One-time proof generation
- Browser or mobile app environments

### What's Missing

#### **1. WebSocket over TLS (WSS)**
- No zkTLS implementation for persistent WebSocket connections
- No proof generation for streaming WebSocket messages
- Challenge: Continuous attestation vs. one-time proofs

#### **2. MQTT over TLS**
- IoT devices commonly use MQTT for pub/sub messaging
- MQTT-TLS connections need cryptographic attestation
- Challenge: Lightweight protocol for constrained devices

#### **3. Real-Time Streaming**
- Current zkTLS is "snapshot" based (prove data at time T)
- Streaming requires continuous or batched verification
- Challenge: Performance overhead for high-frequency data

#### **4. IoT Device Constraints**
- Many IoT devices lack resources for zk-SNARK generation
- Need lightweight attestation mechanisms
- Challenge: Balance between security and device capabilities

### Why This Gap Exists

1. **zkTLS research** has focused on web2-to-web3 data oracles
2. **Primary use cases** have been identity, finance, social media
3. **Browser/mobile environments** were the initial targets
4. **IoT/streaming** represent different threat models and requirements

### Opportunity

This gap represents an opportunity to:
- Extend TLSNotary's MPC-TLS to WebSocket/MQTT protocols
- Build lightweight attestation for IoT devices
- Create streaming proof architectures
- Pioneer zkTLS for industrial/IoT applications

---

## Recommendation

Based on comprehensive analysis of your requirements and the zkTLS landscape, I recommend a **three-tier hybrid approach**:

### 🥇 Tier 1: TLSNotary Foundation (Self-Hosted)

**For:** Custom deployments, IoT extensions, maximum control

**Rationale:**
- Open source with permissive licensing
- Self-hostable notary servers
- Modular Rust codebase enables custom extensions
- Strong cryptographic foundation for building custom protocols
- No vendor lock-in

**Use for:**
- Custom WebSocket-over-TLS verification (extend MPC-TLS)
- MQTT-TLS attestation for IoT devices
- Any novel data source requiring custom integration
- Building your own attestation infrastructure

**Implementation:**
1. Deploy self-hosted TLSNotary notary servers
2. Extend TLSNotary's Rust codebase to support WebSocket/MQTT
3. Create custom notary logic for streaming data
4. Build lightweight client SDKs for IoT devices

---

### 🥈 Tier 2: Reclaim attestor-core (Self-Hosted)

**For:** Rapid deployment of WebView scraping, mobile apps

**Rationale:**
- Production-ready with excellent mobile SDKs
- Fast to integrate (days vs. months)
- 889+ pre-built data sources
- Self-hostable via attestor-core
- Proven at scale (1M+ proofs)

**Use for:**
- Flutter WebView scraping (LinkedIn, etc.)
- React Native mobile applications
- Quick wins while building custom TLSNotary extensions
- Any use case matching Reclaim's provider model

**Implementation:**
1. Deploy self-hosted attestor-core with Docker
2. Use Reclaim's Flutter SDK in your mobile apps
3. Create custom providers for your specific scraping needs
4. Consider Eigen AVS for decentralization if needed

---

### 🔧 Tier 3: Custom Protocol Extensions

**For:** IoT devices, streaming data, WebSocket/MQTT

**Rationale:**
- No existing solution available
- You have unique requirements
- TLSNotary provides cryptographic primitives to build on
- Opportunity to pioneer zkTLS for IoT/streaming

**Approach:**

#### **For WebSocket Streaming:**
```
Base: TLSNotary's MPC-TLS
Extension: Continuous session attestation
Mechanism:
  - Maintain MPC session for duration of WebSocket connection
  - Generate attestations for message batches
  - Use merkle trees for efficient multi-message proofs
  - Notary signs merkle roots periodically
```

#### **For MQTT-TLS:**
```
Base: TLSNotary's notary server
Extension: Lightweight IoT client
Mechanism:
  - Simplified MPC for resource-constrained devices
  - Edge notary servers close to IoT devices
  - Batch attestation of pub/sub messages
  - Aggregate proofs for multiple devices
```

#### **For Native Mobile App Traffic:**
```
Base: OS-level TLS interception (from your previous research)
Integration: Proxy to TLSNotary or Reclaim attestor
Mechanism:
  - Flutter app → VPN/Proxy → Attestor → Internet
  - Certificate pinning bypass (self-signed cert injection)
  - Attestor validates and signs intercepted traffic
  - Generate proofs for specific app data
```

---

## Detailed Implementation Roadmap

### Phase 1: Quick Wins with Reclaim (Weeks 1-4)

**Goal:** Get WebView scraping working immediately

**Tasks:**
1. Deploy self-hosted Reclaim attestor-core
   ```bash
   git clone https://github.com/reclaimprotocol/attestor-core
   cd attestor-core
   docker-compose up -d
   ```

2. Integrate Reclaim Flutter SDK
   ```yaml
   # pubspec.yaml
   dependencies:
     reclaim_sdk: ^latest
   ```

3. Create custom providers for your scraping targets (LinkedIn, etc.)

4. Build proof-of-concept Flutter app with WebView scraping

**Deliverables:**
- Working LinkedIn scraper with cryptographic proofs
- Self-hosted attestor infrastructure
- Documentation for team

**Risk Mitigation:**
- Reclaim is production-ready, low risk
- Can move to TLSNotary later if needed
- Self-hosting avoids vendor dependency

---

### Phase 2: TLSNotary Foundation (Weeks 5-12)

**Goal:** Establish self-hosted TLSNotary infrastructure for long-term customization

**Tasks:**

1. **Deploy TLSNotary Notary Servers**
   ```bash
   git clone https://github.com/tlsnotary/tlsn
   cd tlsn/notary-server
   cargo build --release
   # Configure and deploy
   ```

2. **Set up multiple notaries for redundancy**
   - Deploy 3+ notary servers in different geographic regions
   - Configure load balancing
   - Implement health monitoring

3. **Integrate TLSNotary into Flutter app** (parallel to Reclaim)
   - Build custom Flutter plugin wrapping TLSNotary's WASM
   - Create simplified SDK for mobile developers
   - Benchmark performance vs. Reclaim

4. **Develop internal tooling**
   - Proof verification libraries
   - Monitoring dashboards
   - Logging and analytics

**Deliverables:**
- Production TLSNotary infrastructure
- Custom Flutter integration layer
- Comparative analysis: Reclaim vs. TLSNotary

**Decision Point:**
- Evaluate which to use as primary for WebView scraping
- Consider keeping both: Reclaim for speed, TLSNotary for security-critical

---

### Phase 3: Native Mobile App Support (Weeks 13-20)

**Goal:** Capture and attest traffic from native iOS/Android apps

**Approach:** Build on your previous research (traffic-capture-platform)

**Architecture:**
```
Mobile Device (iOS/Android)
    ↓
  VPN/Proxy Layer
    ↓
  Certificate Injection (bypass pinning)
    ↓
  TLS Traffic Interceptor
    ↓
  Attestor (TLSNotary or Reclaim)
    ↓
  Proof Generation
```

**Tasks:**

1. **Implement OS-level VPN**
   - iOS: NEPacketTunnelProvider
   - Android: VpnService API
   - Route all traffic through your proxy

2. **TLS Interception**
   - Inject your own CA certificate
   - MITM TLS connections (for attestation purposes)
   - Handle certificate pinning (documented in your prior research)

3. **Integration with Attestor**
   - Forward intercepted traffic to TLSNotary notary or Reclaim attestor
   - Generate proofs for specific app data (e.g., API responses)
   - Handle session management

4. **Proof Storage and Verification**
   - Store proofs locally on device
   - Upload to your backend
   - Provide verification API for third parties

**Deliverables:**
- Native mobile VPN app (iOS + Android)
- Traffic capture with proof generation
- Documentation on limitations (cert pinning, etc.)

**Challenges:**
- App Store restrictions on VPN apps
- Certificate pinning bypass detection
- Performance overhead

---

### Phase 4: IoT and Streaming Extensions (Weeks 21-32)

**Goal:** Extend TLSNotary to support WebSocket, MQTT, and streaming data

#### **4A: WebSocket-over-TLS (WSS) Support**

**Challenge:** WebSocket is persistent; traditional zkTLS is one-shot

**Approach:**

1. **Extend TLSNotary's MPC-TLS session**
   - Keep MPC session alive for duration of WebSocket connection
   - Notary maintains state for ongoing connection

2. **Batched Attestation**
   ```
   Time:     t0    t1    t2    t3    t4    t5
   Messages: [m1, m2, m3] [m4, m5, m6] [m7, m8, m9]
                  ↓             ↓             ↓
               Batch 1       Batch 2       Batch 3
                  ↓             ↓             ↓
               Merkle        Merkle        Merkle
                Root 1        Root 2        Root 3
                  ↓             ↓             ↓
               Notary        Notary        Notary
              Signature     Signature     Signature
   ```

3. **Proof Structure**
   - Notary signs merkle roots every N seconds or M messages
   - Client can prove any message by providing merkle path
   - Maintains privacy (only reveal specific messages)

**Implementation:**
```rust
// Pseudo-code extension to TLSNotary
struct StreamingSession {
    mpc_session: TlsNotarySession,
    message_buffer: Vec<Message>,
    batch_interval: Duration,
    merkle_tree: MerkleTree,
}

impl StreamingSession {
    async fn attest_batch(&mut self) -> NotarySignature {
        let root = self.merkle_tree.root();
        self.mpc_session.notary_sign(root).await
    }

    fn prove_message(&self, msg: Message) -> Proof {
        let path = self.merkle_tree.get_path(msg);
        Proof { msg, path, notary_sig: self.batch_sig }
    }
}
```

---

#### **4B: MQTT-over-TLS Support**

**Challenge:** IoT devices are resource-constrained

**Approach:**

1. **Edge Notary Architecture**
   ```
   IoT Device (MQTT client)
        ↓
   Edge Notary Server (local network)
        ↓
   MQTT Broker
        ↓
   Central Verification Service
   ```

2. **Lightweight Client Protocol**
   - Simplified MPC for low-power devices
   - Attestor does heavy lifting (proof generation)
   - Device only needs to participate in key exchange

3. **Aggregate Proofs**
   - One notary handles multiple IoT devices
   - Batch proofs for efficiency
   - Periodic synchronization to central service

**Implementation:**
- Fork TLSNotary's notary-server
- Add MQTT protocol support
- Create lightweight client SDK (C/C++ for embedded)
- Optimize for ARM Cortex-M, ESP32, etc.

---

#### **4C: Real-Time Streaming Data**

**Challenge:** High-frequency data (e.g., sensor streams)

**Approach:**

1. **Sliding Window Attestation**
   ```
   Time: ──────────────────────────────────────→
         [    Window 1    ]
              [    Window 2    ]
                   [    Window 3    ]

   Each window:
   - Contains N data points
   - Generates single attestation
   - Overlaps with previous window (for continuity proof)
   ```

2. **Statistical Commitments**
   - Instead of attesting every data point, attest to statistical properties
   - E.g., "Average temperature over 5 minutes was 23.4°C ± 0.2°C"
   - Use ZK proofs to prove statistics without revealing all points

3. **Merkle Clock Trees**
   - Merkle trees with timestamps
   - Prove data existed at specific time
   - Chain attestations for continuity

---

### Phase 5: Integration and Optimization (Weeks 33-40)

**Goal:** Unify all components into cohesive platform

**Tasks:**

1. **Unified SDK**
   - Single SDK supporting: WebView, Native Apps, IoT, Streaming
   - Consistent API across use cases
   - Documentation and examples

2. **Verification Infrastructure**
   - Public verification API
   - Smart contract verifiers (if needed)
   - Dashboard for proof analytics

3. **Performance Optimization**
   - Benchmark all proof generation paths
   - Optimize for mobile (battery, bandwidth)
   - Caching and batch processing

4. **Security Audit**
   - Engage external auditors
   - Penetration testing
   - Formal verification of critical paths

**Deliverables:**
- Production-ready platform
- Comprehensive documentation
- Security audit report

---

## Why Not Just Use Reclaim?

Your question was specifically about replacing Reclaim. Here's why:

### Limitations of Reclaim for Your Use Case

1. **No IoT/Streaming Support**
   - Reclaim is HTTPS-only
   - No support for MQTT, WebSocket, or real-time streams
   - Would require significant custom development

2. **Trust Model**
   - Single attestor by default (must trust Reclaim or your hosted attestor)
   - Decentralization via Eigen AVS is new and unproven at scale
   - For high-security use cases, MPC (TLSNotary) is stronger

3. **Limited Customization**
   - While attestor-core is open source, core protocol isn't fully customizable
   - Provider model works great for web scraping, less so for novel data sources
   - TypeScript may not be ideal for embedded/IoT (need C/Rust)

4. **Vendor Dependency**
   - Even self-hosted, you're dependent on Reclaim's architecture
   - Future changes to protocol could affect your deployments
   - Limited ability to innovate beyond Reclaim's roadmap

### When Reclaim IS the Right Choice

**Use Reclaim (attestor-core) when:**
- You need fast time-to-market for WebView scraping
- Your data sources are standard HTTPS websites
- You want to leverage 889+ pre-built providers
- Mobile SDK quality matters more than cryptographic purity
- Your trust model accepts single attestor (or small attestor set)

**Use TLSNotary when:**
- You need maximum customization
- IoT, streaming, or novel protocols are required
- Strongest cryptographic guarantees are essential
- You want to build your own zkTLS infrastructure
- Long-term control and sovereignty matter

### Recommended Hybrid Approach

**Best of both worlds:**
- Use Reclaim attestor-core for **rapid deployment** of WebView scraping (Phase 1)
- Build on TLSNotary for **custom requirements** (Phases 2-4)
- Maintain both: Reclaim for "standard" use cases, TLSNotary for advanced

This gives you:
- ✅ Fast time-to-value (Reclaim)
- ✅ Future-proof customization (TLSNotary)
- ✅ No single point of failure
- ✅ Flexibility to migrate between systems

---

## Architecture Comparison Table

| Feature | Reclaim (Self-Hosted) | TLSNotary | Hybrid Approach |
|---------|----------------------|-----------|-----------------|
| **Time to Production** | 🟢 1-2 weeks | 🟡 1-2 months | 🟢 Reclaim first, then TLSNotary |
| **WebView Scraping** | 🟢 Excellent (Flutter SDK) | 🟡 Custom integration needed | 🟢 Use Reclaim |
| **Native App Traffic** | 🟡 Possible with proxy | 🟡 Possible with proxy | 🟢 Either works |
| **IoT Devices** | 🔴 Not supported | 🟡 Requires custom extension | 🟢 Extend TLSNotary |
| **WebSocket/MQTT** | 🔴 Not supported | 🟡 Requires custom extension | 🟢 Extend TLSNotary |
| **Cryptographic Security** | 🟡 Proxy model (trust attestor) | 🟢 MPC (no single trust point) | 🟢 Use TLSNotary for critical paths |
| **Self-Hosting Ease** | 🟢 Docker compose | 🟡 Rust compilation | 🟢 Both self-hostable |
| **Customization** | 🟡 Provider framework | 🟢 Fully modular Rust | 🟢 Best of both |
| **Mobile SDKs** | 🟢 Flutter, React Native | 🔴 None (build your own) | 🟢 Use Reclaim SDKs |
| **Community Support** | 🟢 Active, 889+ providers | 🟢 EF-backed, active | 🟢 Both |
| **License** | 🟡 Mixed (some proprietary) | 🟢 Apache2/MIT | 🟢 TLSNotary for critical |
| **Decentralization** | 🟡 Eigen AVS (new) | 🟢 Multi-notary support | 🟢 Use TLSNotary |
| **TLS Version Support** | 🟢 TLS 1.2 & 1.3 | 🟡 TLS 1.2 only | 🟡 Limited by TLSNotary |
| **Proof Generation Speed** | 🟢 2-4s on mobile | 🟡 Slower (more computation) | 🟢 Use Reclaim for speed |
| **Verification Cost** | 🟢 Fast | 🟢 Fast | 🟢 Both efficient |

**Legend:** 🟢 Excellent | 🟡 Good/Acceptable | 🔴 Poor/Not Supported

---

## Risk Assessment

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **TLSNotary doesn't support TLS 1.3** | High | Medium | Monitor roadmap; most sites still support 1.2 |
| **IoT extensions are complex** | High | High | Phase approach; start with WebView (lower risk) |
| **Performance overhead on IoT** | Medium | High | Edge notaries, lightweight protocols, testing |
| **Mobile app store restrictions** | Medium | High | Compliance review, avoid VPN if possible |
| **Certificate pinning blocks capture** | High | Medium | Documented bypass techniques (prior research) |
| **Proof generation battery drain** | Medium | Medium | Optimize, batch proofs, edge computing |
| **Reclaim dependency** | Low | Medium | Self-host attestor-core, contribute upstream |

### Operational Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Notary/Attestor downtime** | Medium | High | Multiple notaries, redundancy, monitoring |
| **Scaling proof generation** | Medium | Medium | Horizontal scaling, caching, batching |
| **Key management** | Low | Critical | HSMs, key rotation, security audit |
| **Team zkTLS expertise** | High | Medium | Training, consultants, community support |

### Strategic Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Reclaim changes protocol** | Medium | Medium | Self-host attestor-core, version pinning |
| **TLSNotary development stalls** | Low | High | EF-backed, active community, can fork |
| **Regulatory changes** | Low | High | Legal review, privacy compliance (GDPR) |
| **Competitor builds IoT zkTLS** | Medium | Low | First-mover advantage, rapid Phase 4 |

---

## Cost Estimate

### Self-Hosted Infrastructure (Annual)

**Reclaim attestor-core:**
- Compute: 3 servers × $100/month = $3,600/year
- Load balancer: $500/year
- Monitoring: $1,000/year
- **Total: ~$5,100/year**

**TLSNotary Notaries:**
- Compute: 5 servers × $150/month = $9,000/year
- Load balancer: $500/year
- Monitoring: $1,000/year
- **Total: ~$10,500/year**

**IoT Edge Notaries (Phase 4):**
- Edge devices: 10 locations × $50/month = $6,000/year
- **Total: ~$6,000/year**

**Grand Total Infrastructure: ~$21,600/year** (all phases)

Compare to:
- Reclaim SaaS (if available): Likely $50K-$200K+/year at scale
- Building from scratch: $500K+ in development costs

**ROI:** Self-hosting pays for itself vs. SaaS, plus you own the infrastructure.

---

## Alternative Architectures Considered

### Option A: Pure Reclaim (Self-Hosted)
**Pros:** Fastest deployment, excellent mobile SDKs
**Cons:** No IoT/streaming support, trust assumptions, limited customization
**Verdict:** ❌ Insufficient for full requirements

### Option B: Pure TLSNotary (Custom Build)
**Pros:** Maximum control, strongest security, fully customizable
**Cons:** Slow to market, no mobile SDKs, significant development
**Verdict:** ⚠️ High risk, high reward

### Option C: Hybrid (Recommended)
**Pros:** Fast time-to-market + long-term flexibility, best of both
**Cons:** Maintaining two systems initially
**Verdict:** ✅ **Recommended**

### Option D: Wait for zkPass/ExpandZK
**Pros:** Might be better fit when mature
**Cons:** Timeline uncertain, no IoT focus yet
**Verdict:** ❌ Too risky to wait

### Option E: Build from Scratch
**Pros:** Total control
**Cons:** Months/years of cryptography development, security risks
**Verdict:** ❌ Not practical

---

## Conclusion

### Summary of Recommendations

1. **Immediate (Weeks 1-4):** Deploy self-hosted Reclaim attestor-core for WebView scraping. Get to production fast with proven technology.

2. **Foundation (Weeks 5-12):** Stand up self-hosted TLSNotary notary servers. Build the infrastructure for long-term customization.

3. **Expansion (Weeks 13-20):** Tackle native mobile app traffic capture, leveraging your prior research.

4. **Innovation (Weeks 21-32):** Extend TLSNotary for IoT, WebSocket, and MQTT. Pioneer zkTLS for streaming data.

5. **Production (Weeks 33-40):** Unify, optimize, audit, and launch.

### Why This Approach Works

✅ **Addresses all your use cases:** WebView, native apps, IoT, streaming
✅ **Balances speed and customization:** Reclaim for quick wins, TLSNotary for innovation
✅ **Self-hosted:** No vendor lock-in, full control
✅ **Open source:** Permissive licenses, can fork if needed
✅ **Proven technology:** Building on production systems (Reclaim 1M+ proofs, TLSNotary by EF)
✅ **Future-proof:** Modular architecture allows evolution
✅ **Cryptographically sound:** MPC-based TLSNotary for strongest guarantees

### Final Verdict

**Replace Reclaim with:** TLSNotary as the long-term foundation
**But keep Reclaim for:** Rapid deployment and excellent mobile SDKs
**Custom build:** IoT and streaming extensions on TLSNotary

This hybrid approach gives you:
- Production today (Reclaim)
- Flexibility tomorrow (TLSNotary)
- Innovation for IoT/streaming (custom extensions)

You're not just replacing Reclaim—you're building a comprehensive, custom zkTLS platform that addresses your unique requirements while standing on the shoulders of giants.

---

## Next Steps

1. **Review this research** with your technical team
2. **Validate assumptions** about your specific data sources
3. **Prototype Phase 1** (Reclaim attestor-core deployment)
4. **Hire/train** on zkTLS cryptography (or engage consultants)
5. **Engage communities** - TLSNotary Discord, Reclaim, PSE
6. **Plan security audit** for Phase 5

---

## References

- TLSNotary: https://tlsnotary.org
- TLSNotary GitHub: https://github.com/tlsnotary/tlsn
- Reclaim Protocol: https://reclaimprotocol.org
- Reclaim attestor-core: https://github.com/reclaimprotocol/attestor-core
- vlayer: https://vlayer.xyz
- DECO Paper: https://arxiv.org/abs/1909.00938
- TLSNotary Review: https://arxiv.org/abs/2409.17670
- zkTLS Day 2025: https://tlsnotary.org/zktls-day/

---

**Research conducted by:** AI Research Agent
**Date:** November 14, 2025
**Version:** 1.0
