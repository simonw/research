# Detailed zkTLS Protocol Comparison

## Technical Architecture Comparison

### TLSNotary (MPC-Based)

**Protocol Flow:**
```
┌─────────────┐         ┌──────────────┐         ┌─────────────┐
│   Prover    │◄───────►│    Notary    │         │   Server    │
│  (Client)   │         │  (Verifier)  │         │             │
└─────────────┘         └──────────────┘         └─────────────┘
       │                        │                        │
       │                        │                        │
       │  1. MPC Key Exchange   │                        │
       ├───────────────────────►│                        │
       │  (Split TLS keys)      │                        │
       │◄───────────────────────┤                        │
       │                        │                        │
       │  2. TLS Handshake (joint)                       │
       ├─────────────────────────────────────────────────┤
       │                        │                        │
       │  3. Encrypted Request  │                        │
       ├─────────────────────────────────────────────────►
       │                        │                        │
       │  4. Encrypted Response │                        │
       ◄─────────────────────────────────────────────────┤
       │                        │                        │
       │  5. Selective Disclosure (redact sensitive)     │
       │                        │                        │
       │  6. Generate ZK Proof  │                        │
       │                        │                        │
       │  7. Request Signature  │                        │
       ├───────────────────────►│                        │
       │                        │                        │
       │  8. Notary Signature   │                        │
       ◄───────────────────────┤                        │
       │                        │                        │
       │  9. Final Proof = {data, ZK proof, signature}   │
```

**Key Properties:**
- **Trust Model:** No single party needs to be trusted
- **Key Splitting:** Prover holds decryption key share, Notary holds MAC key share
- **Forgery Protection:** Neither party alone can forge TLS data
- **Privacy:** Notary never sees plaintext (unless disclosed)
- **Verification:** Anyone with notary's public key can verify proofs

**Cryptographic Primitives:**
- Garbled Circuits (Yao's protocol)
- Oblivious Transfer (OT)
- AES-GCM for TLS encryption
- HMAC for TLS message authentication
- Optional: zk-SNARKs for selective disclosure

---

### Reclaim Protocol (Proxy-Based)

**Protocol Flow:**
```
┌─────────────┐         ┌──────────────┐         ┌─────────────┐
│   Client    │         │   Attestor   │         │   Server    │
│             │         │   (Proxy)    │         │             │
└─────────────┘         └──────────────┘         └─────────────┘
       │                        │                        │
       │                        │                        │
       │  1. TLS to Attestor    │                        │
       ├───────────────────────►│                        │
       │  (User keeps decrypt)  │                        │
       │                        │                        │
       │                        │  2. TLS to Server      │
       │                        ├───────────────────────►│
       │                        │  (Attestor sees cipher)│
       │                        │                        │
       │                        │  3. Response (cipher)  │
       │                        ◄───────────────────────┤
       │                        │                        │
       │  4. Response + Session │                        │
       ◄───────────────────────┤                        │
       │     Metadata           │                        │
       │                        │                        │
       │  5. Create Claim       │                        │
       │  (structured data)     │                        │
       │                        │                        │
       │  6. Submit Claim       │                        │
       ├───────────────────────►│                        │
       │                        │                        │
       │  7. Validate Claim     │                        │
       │     (decrypt needed    │                        │
       │      portions only)    │                        │
       │                        │                        │
       │  8. Sign Claim         │                        │
       │                        │                        │
       │  9. Attestation        │                        │
       ◄───────────────────────┤                        │
       │                        │                        │
       │  10. Generate ZK Proof │                        │
       │   (optional, on-device)│                        │
```

**Key Properties:**
- **Trust Model:** Must trust attestor not to collude with user
- **Key Control:** User controls decryption, attestor validates ciphertext authenticity
- **Selective Sharing:** User chooses what to decrypt for attestor
- **Speed:** Lower latency than MPC (no multi-round protocol)
- **Decentralization:** Can use multiple attestors (quorum)

**Cryptographic Primitives:**
- Standard TLS 1.2/1.3
- Groth16 zk-SNARKs for selective disclosure
- ECDSA signatures from attestor
- Merkle trees for data commitments

---

## Feature Comparison Matrix

| Feature | TLSNotary | Reclaim | Opacity | DECO (Chainlink) | zkPass | vlayer |
|---------|-----------|---------|---------|------------------|--------|--------|
| **Architecture** | MPC (2PC) | Proxy | MPC + AVS | MPC (3PC) | Hybrid MPC+ZK | TLSNotary + Ethereum |
| **Trust Model** | Trustless (MPC) | Semi-trusted proxy | Economic security | Trustless | Semi-trustless | Trustless (TLSNotary) |
| **TLS Version** | 1.2 | 1.2 & 1.3 | 1.2 & 1.3 | 1.2 & 1.3 | 1.2 & 1.3 | 1.2 (TLSNotary) |
| **Proof Type** | Signature + optional ZK | Groth16 SNARK | SNARK + signatures | Signature + ZK | VOLE-IZK → SNARK | SNARK |
| **Proof Gen Time** | ~10-30s | 2-4s | ~5-10s | Unknown | <1s (claimed) | ~10-30s |
| **Mobile SDK** | ❌ No | ✅ Flutter, RN | 🟡 Coming | ❌ No | 🟡 Coming | ❌ No |
| **Self-Hosted** | ✅ Yes | ✅ Yes (attestor) | ❌ No | ❌ No | 🟡 Limited | ✅ Yes |
| **Open Source** | ✅ Full | 🟡 Partial | ❌ No | ❌ No | 🟡 Partial | ✅ Full |
| **License** | Apache2/MIT | Mixed | Proprietary | Proprietary | Mixed | Apache2/MIT |
| **Language** | Rust | TypeScript | Unknown | Unknown | Unknown | Rust/TS |
| **Notary Count** | 1 (2PC limit) | 1+ (configurable) | 5+ (network) | 1-3 | Multiple | 1 (TLSNotary) |
| **Decentralization** | Can run multiple | Eigen AVS | EigenLayer AVS | Chainlink network | MPC nodes | Can run multiple |
| **On-Chain Verify** | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes (SBT) | ✅ Yes (Ethereum) |
| **Selective Disclosure** | ✅ Yes (ZK) | ✅ Yes (ZK) | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes |
| **Pre-Built Providers** | ❌ No | ✅ 889+ | Unknown | 🟡 Templates | Unknown | ❌ No |
| **Maturity** | Production | Production | Pilot | Sandbox | Beta | Mainnet (2025) |
| **Audited** | ❌ No | 🟡 Partial | ❌ No | ❌ No | ❌ No | ❌ No |
| **Community** | Large | Large | Small | Large (Chainlink) | Medium | Growing |
| **Documentation** | Excellent | Excellent | Limited | Good | Good | Good |
| **WebView Support** | 🟡 Custom | ✅ Native | Unknown | ❌ No | 🟡 Coming | 🟡 Custom |
| **Browser Extension** | ✅ Yes | ✅ Yes | Unknown | ❌ No | 🟡 Coming | ✅ Yes (TLSNotary) |
| **TEE Support** | ✅ SGX optional | ❌ No | 🟡 Possible | 🟡 Possible | ❌ No | ✅ SGX optional |
| **Proving System** | Garbled Circuits | Groth16 | Unknown | GC + SNARK | VOLE-IZK | Garbled Circuits |
| **Batch Proofs** | ❌ No | ✅ Yes | Unknown | Unknown | ✅ Yes | ❌ No |
| **WebSocket Support** | ❌ No | ❌ No | ❌ No | ❌ No | ❌ No | ❌ No |
| **MQTT Support** | ❌ No | ❌ No | ❌ No | ❌ No | ❌ No | ❌ No |
| **IoT Focused** | ❌ No | ❌ No | ❌ No | ❌ No | ❌ No | ❌ No |

**Legend:**
- ✅ Fully supported
- 🟡 Partial/In development
- ❌ Not supported

---

## Security Model Comparison

### Attack Scenarios

| Attack | TLSNotary | Reclaim | Opacity | DECO |
|--------|-----------|---------|---------|------|
| **Malicious User (forge data)** | ✅ Prevented (needs notary) | 🟡 Prevented (needs attestor) | ✅ Prevented (needs quorum) | ✅ Prevented (MPC) |
| **Malicious Notary/Attestor** | ✅ Cannot forge (no decrypt) | 🔴 Can collude with user | 🟡 Economic disincentive | ✅ Cannot forge |
| **MITM Attack** | ✅ Prevented (TLS) | ✅ Prevented (TLS) | ✅ Prevented | ✅ Prevented |
| **Replay Attack** | ✅ Prevented (timestamps) | ✅ Prevented (nonces) | ✅ Prevented | ✅ Prevented |
| **Notary Collusion (multi-notary)** | 🟡 Possible if all collude | 🟡 Possible if quorum | 🟡 Slashed if detected | 🟡 Possible if all collude |
| **Privacy Breach (notary sees data)** | ✅ Prevented (MPC) | ✅ Prevented (selective decrypt) | ✅ Prevented | ✅ Prevented |
| **Sybil (fake notaries)** | 🟡 Public key trust | 🟡 Attestor whitelist | ✅ Prevented (staking) | 🟡 Chainlink reputation |

---

## Performance Benchmarks (Estimated)

| Metric | TLSNotary | Reclaim | Opacity | DECO | zkPass |
|--------|-----------|---------|---------|------|--------|
| **Proof Generation** | 10-30s | 2-4s | ~5-10s | Unknown | <1s* |
| **Proof Size** | ~100KB | ~5KB (Groth16) | ~10KB | Unknown | ~5KB |
| **Bandwidth (MPC)** | ~10MB | ~1MB | ~5MB | Unknown | ~2MB |
| **CPU (Prover)** | Medium | Low | Medium | Unknown | Low* |
| **CPU (Notary)** | High | Low | Medium | Unknown | Medium |
| **Memory (Prover)** | ~100MB | ~50MB | ~100MB | Unknown | ~50MB |
| **Battery Impact (Mobile)** | Medium-High | Low | Medium | Unknown | Low |
| **Latency Added** | +500ms | +100ms | +200ms | Unknown | +50ms |
| **Concurrent Sessions** | 100s | 1000s | 100s | Unknown | 1000s |

*zkPass claims, not independently verified

---

## Cost Analysis (Self-Hosted)

### TLSNotary Infrastructure

**Single Notary Server:**
- CPU: 4-8 cores
- RAM: 16-32 GB
- Storage: 100 GB SSD
- Bandwidth: 1 TB/month
- Cost: ~$150/month (AWS c5.2xlarge equivalent)

**For 100,000 proofs/month:**
- Compute: 5 notaries × $150 = $750/month
- Load balancer: $50/month
- Monitoring: $100/month
- **Total: ~$900/month = $10,800/year**

**Cost per proof:** $0.009

---

### Reclaim attestor-core Infrastructure

**Single Attestor Server:**
- CPU: 2-4 cores
- RAM: 8-16 GB
- Storage: 50 GB SSD
- Bandwidth: 500 GB/month
- Cost: ~$100/month (AWS c5.large equivalent)

**For 100,000 proofs/month:**
- Compute: 3 attestors × $100 = $300/month
- Load balancer: $50/month
- Monitoring: $100/month
- **Total: ~$450/month = $5,400/year**

**Cost per proof:** $0.0045

---

### Comparison to SaaS (Estimated)

If Reclaim offered SaaS pricing (hypothetical):
- Startup tier: $500/month (10K proofs)
- Growth tier: $2,000/month (100K proofs)
- Enterprise: $10,000+/month (1M+ proofs)

**Break-even:** Self-hosting saves money at >10K proofs/month

---

## Integration Complexity

| Task | TLSNotary | Reclaim | Effort Multiplier |
|------|-----------|---------|-------------------|
| **Deploy Infrastructure** | Medium | Easy | 1.5x |
| **Browser Integration** | Easy (extension) | Easy (SDK) | 1x |
| **Mobile Integration** | Hard (custom) | Easy (SDK) | 5x |
| **Custom Provider/Site** | Medium | Easy (template) | 2x |
| **Proof Verification** | Easy | Easy | 1x |
| **Multi-Notary Setup** | Medium | Easy | 1.5x |
| **Monitoring/Ops** | Medium | Medium | 1x |
| **Security Hardening** | Hard | Medium | 2x |

**Estimated Development Time:**

**TLSNotary:**
- Infrastructure: 2 weeks
- Basic integration: 4 weeks
- Production-ready: 8-12 weeks
- Custom extensions (IoT): +12 weeks

**Reclaim:**
- Infrastructure: 1 week
- Basic integration: 1-2 weeks
- Production-ready: 3-4 weeks
- Custom providers: +2 weeks

**Hybrid:**
- Start with Reclaim: 3-4 weeks to production
- Add TLSNotary: +8 weeks in parallel
- Custom extensions: +12 weeks

---

## Decentralization Comparison

### TLSNotary
- **Model:** Can deploy multiple independent notaries
- **Trust:** User chooses which notary to use
- **Coordination:** No coordination between notaries (single-notary per session)
- **Limitation:** Garbled circuits are 2-party, so max 1 notary per proof
- **Workaround:** User can generate multiple proofs with different notaries

### Reclaim
- **Model:** Multiple attestors in a network
- **Trust:** Quorum of attestors (e.g., 3 of 5)
- **Coordination:** Eigen AVS for economic security
- **Slashing:** Misbehavior penalized via staked tokens
- **Flexibility:** Can configure trust threshold

### Opacity
- **Model:** Distributed notary network via EigenLayer AVS
- **Trust:** Economic incentives + slashing
- **Coordination:** Restaking via EigenLayer
- **Security:** Strongest decentralization + crypto-economic guarantees
- **Status:** In development

### DECO (Chainlink)
- **Model:** Chainlink oracle network
- **Trust:** Chainlink's reputation system
- **Coordination:** Off-chain computation, on-chain attestation
- **Security:** Multiple oracle nodes sign
- **Status:** Sandbox, not decentralized yet

---

## Recommendation by Use Case

### WebView Scraping (LinkedIn, Twitter, etc.)
**Winner:** 🥇 Reclaim
- Reason: Flutter/React Native SDKs, 889+ providers, fast

### Native Mobile App Traffic
**Winner:** 🥇 TLSNotary or Reclaim (tie)
- Reason: Both require custom proxy layer

### IoT Devices
**Winner:** 🥇 TLSNotary (with custom extensions)
- Reason: Rust codebase, can extend for MQTT, lightweight clients

### WebSocket/Streaming
**Winner:** 🥇 TLSNotary (with custom extensions)
- Reason: Only option that's extensible enough

### Maximum Security (Banking, Healthcare)
**Winner:** 🥇 TLSNotary
- Reason: Strongest cryptographic guarantees (MPC)

### Fastest Time to Market
**Winner:** 🥇 Reclaim
- Reason: Production-ready, excellent SDKs

### Self-Sovereign Infrastructure
**Winner:** 🥇 TLSNotary
- Reason: Fully open source, permissive license

### Ethereum Integration
**Winner:** 🥇 vlayer
- Reason: Built for Ethereum, mainnet live

---

## Future Roadmap Comparison

### TLSNotary (2025-2026)
- ✅ TLS 1.3 support
- ✅ Multi-notary coordination improvements
- ✅ Mobile SDKs (community-driven)
- ⚠️ No IoT/streaming plans announced

### Reclaim (2025-2026)
- ✅ Eigen AVS full rollout
- ✅ More blockchain integrations
- ✅ AI-powered provider generation
- ⚠️ No IoT/streaming plans announced

### Opacity (2025-2026)
- ✅ Full EigenLayer AVS launch
- ✅ Developer portal public release
- ✅ Production with real apps
- ⚠️ No IoT focus

### DECO/Chainlink (2025-2026)
- ✅ Integration into Chainlink oracle network
- ✅ Enterprise partnerships
- ✅ Compliance focus (KYC/AML)
- ⚠️ Likely enterprise-only

---

## Conclusion

**For your use case**, the **hybrid TLSNotary + Reclaim approach** is optimal because:

1. **Reclaim** covers WebView scraping needs immediately (production-ready)
2. **TLSNotary** provides foundation for IoT/streaming extensions (customizable)
3. **Both** are self-hostable (sovereignty)
4. **Neither** alone addresses all requirements (gap in IoT/streaming)
5. **Custom extensions** on TLSNotary fill the gaps

This combination minimizes risk while maximizing flexibility.
