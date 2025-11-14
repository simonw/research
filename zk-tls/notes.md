# ZK-TLS Research Notes

## Project Goal
Replace Reclaim Protocol with a more custom solution for our data ingestion pipeline.

## Use Cases to Address
1. **Web scraping via Flutter WebView**: Custom JS injection to scrape data from sites like LinkedIn
2. **Native mobile apps**: Traffic capture from iOS/Android apps
3. **IoT devices**: Data provenance from IoT sources
4. **Streaming sources**: WebSockets, MQTT, real-time data streams
5. **Core requirement**: Cryptographic proof that data originated from claimed source

## Research Started
- Date: 2025-11-14
- Focus: Evaluating zkTLS solutions for self-hosted/custom deployment

## Initial Observations from Provided Research
The user provided extensive research comparing:
- Opacity Network
- TLSNotary
- Reclaim Protocol
- Chainlink DECO
- ExpandZK
- zkPass

Key insights from their research:
- **Reclaim**: Most production-ready, 1M+ proofs, single attestor model, fast (4s on mobile)
- **TLSNotary**: Open-source foundation, requires more custom integration work
- **Opacity**: Heavy security focus, uses MPC + EigenLayer, still early stage
- **DECO**: Chainlink-backed, enterprise focus, sandbox stage
- **zkPass/ExpandZK**: Emerging solutions, not yet production-ready

## Research Questions to Answer
1. Which solution best supports custom deployment scenarios (WebView, IoT, streaming)?
2. Which solutions can be self-hosted or customized for our pipeline?
3. What are the trade-offs for each use case we have?
4. Which solution provides the best balance of:
   - Flexibility for custom integration
   - Self-hosting capability
   - Support for diverse data sources
   - Cryptographic security guarantees
   - Development velocity

## Investigation Plan
1. ✓ Review provided research thoroughly
2. ✓ Research WebView integration capabilities
3. ✓ Research IoT/streaming support in zkTLS
4. ✓ Evaluate self-hosting options
5. ✓ Compare architectures for custom pipeline integration
6. ✓ Make recommendation with reasoning

## Key Research Findings

### WebView Integration
- **Reclaim Protocol** has mature Flutter SDK (reclaim_sdk on pub.dev) and React Native SDK
- Reclaim uses in-app webview with modified TLS library for proof generation
- Can generate proofs in 2-4 seconds on mobile devices
- Supports gnarkprover Plugin for native proof generation
- TLSNotary has browser extension but less mobile-native integration

### Self-Hosting Capabilities
**TLSNotary:**
- Fully open source (Apache 2 / MIT dual license)
- Can self-host notary server
- Written in Rust, compiles to WASM
- Modular architecture for custom integration
- Supports TEE (SGX) for enhanced security
- Limitation: Currently TLS 1.2 only (TLS 1.3 on roadmap)

**Reclaim Protocol:**
- attestor-core is open source on GitHub
- Can run attestor server locally or deploy to cloud
- Docker support with docker-compose
- TypeScript implementation
- Provider framework for custom data sources
- Uses Eigen AVS for decentralization

**vlayer:**
- Built on top of TLSNotary
- Mainnet launched 2025 with 480K+ proofs
- Focused on Ethereum smart contract integration
- Web Proofs Demo available

### Architecture Comparison

**MPC-Based (TLSNotary, DECO, PADO):**
- Uses Multi-Party Computation
- Garbled circuits approach
- Typically limited to 2-party (one notary)
- Higher security guarantees
- More networking overhead
- No single point of trust

**Proxy-Based (Reclaim):**
- Attestor sits between client and server
- Lower latency (better for real-time)
- Faster proof generation
- Can be decentralized via multiple attestors
- Originally from DECO paper, implemented by Reclaim

### IoT and Streaming Data
- **No existing zkTLS implementations found** specifically for IoT protocols (MQTT, WebSocket, etc.)
- Current zkTLS focuses on HTTPS/TLS web traffic
- This is a **gap in the market** - opportunity for custom implementation
- Would need to extend existing protocols to support:
  - WebSocket over TLS proof generation
  - MQTT over TLS (MQTT-TLS) verification
  - Streaming data provenance
  - IoT device attestation

### Use Case Analysis

**For Flutter WebView scraping (LinkedIn, etc.):**
- ✅ Reclaim Protocol is production-ready with Flutter SDK
- ✅ TLSNotary could work but requires more custom integration
- ⚠️ Both designed for HTTPS, not specialized for scraping

**For Native Mobile Apps:**
- ✅ Reclaim has best mobile support
- ✅ TLSNotary has browser support, less mobile-native
- ⚠️ May need traffic capture layer (similar to our previous research)

**For IoT Devices:**
- ❌ No existing zkTLS solution
- 🔨 Would need custom implementation
- 💡 Could extend TLSNotary or Reclaim for MQTT-TLS, WSS

**For WebSocket/MQTT Streaming:**
- ❌ No existing zkTLS solution
- 🔨 Would need custom protocol extension
- 💡 Core cryptography could be reused from TLSNotary/Reclaim

## Trade-off Analysis

### TLSNotary Strengths:
- Fully open source, permissive licensing
- No trust in single attestor needed
- Modular, customizable architecture
- Active development (EF PSE team)
- Can self-host completely
- Strong security model (MPC-based)

### TLSNotary Weaknesses:
- Requires more integration work
- Less mobile-native than Reclaim
- Limited to one notary (garbled circuits)
- TLS 1.2 only (currently)
- Higher networking overhead

### Reclaim Strengths:
- Production-ready (1M+ proofs)
- Excellent mobile SDKs (Flutter, React Native)
- Fast proof generation (2-4s on mobile)
- 889+ community data sources
- Easy to integrate
- Can self-host attestor-core

### Reclaim Weaknesses:
- Default single attestor (trust assumption)
- Less cryptographically "pure" than MPC
- Proxy model may not suit all use cases
- Decentralization via Eigen AVS is new

### vlayer Strengths:
- Built on TLSNotary (inherits security)
- Strong Ethereum integration
- Mainnet with real usage
- Web Proofs demo available

### vlayer Weaknesses:
- Focused on Ethereum specifically
- Less general-purpose than TLSNotary
- Newer (2025 mainnet launch)

## Critical Gap: IoT and Streaming
**Major finding**: Current zkTLS solutions are HTTPS-focused. There is **no existing production solution** for:
- IoT device data provenance
- WebSocket streaming proofs
- MQTT message verification
- Real-time streaming data attestation

This means we would need to **extend existing protocols** or **build custom components** for IoT/streaming use cases.

## Recommendation Direction
Based on research, I'm leaning toward:
1. **TLSNotary (self-hosted)** as the foundation for custom deployment
2. **Reclaim attestor-core** for rapid WebView integration
3. **Custom protocol extensions** for IoT/streaming (building on TLSNotary crypto)
4. **Hybrid approach** depending on use case requirements

## Final Thoughts

After extensive research, the **hybrid approach is clearly the best path forward**:

### Why Not Pure Reclaim?
- No IoT/streaming support (HTTPS-only)
- Trust model requires trusting attestor(s)
- Limited customization for novel protocols
- TypeScript codebase not ideal for embedded/IoT

### Why Not Pure TLSNotary?
- Slower time-to-market (months vs. weeks)
- No mobile SDKs (need to build custom)
- More complex integration
- Higher development cost upfront

### Why Hybrid Works?
✅ **Fast wins:** Reclaim for WebView scraping (production in weeks)
✅ **Long-term flexibility:** TLSNotary for customization
✅ **Gap filling:** Custom extensions for IoT/streaming
✅ **No vendor lock-in:** Both are self-hostable
✅ **Future-proof:** Can evolve as needed

### Critical Insight: The IoT Gap
The most important finding from this research is that **no existing zkTLS solution supports IoT or streaming protocols**. This means:
- We MUST build custom extensions regardless of which we choose
- TLSNotary's Rust codebase is better suited for this than Reclaim's TypeScript
- This is both a challenge and an opportunity (first-mover advantage)

### Estimated Timeline
- **Week 1-4:** Reclaim POC (WebView scraping)
- **Week 5-12:** TLSNotary foundation
- **Week 13-20:** Native app traffic
- **Week 21-32:** IoT/streaming extensions
- **Week 33-40:** Production hardening

### Estimated Cost
- **Infrastructure:** ~$20K/year (self-hosted)
- **Development:** ~3-6 engineer-months
- **Compare to:** Building from scratch ($500K+) or SaaS ($50K-$200K/year)

**ROI:** Self-hosting hybrid approach is cost-effective and future-proof.

## Research Completed
Date: November 14, 2025
Time spent: ~4 hours of research and analysis
Documents created:
- README.md (comprehensive analysis and recommendation)
- protocol-comparison.md (detailed technical comparison)
- quickstart.md (practical implementation guide)
- notes.md (research process tracking)
