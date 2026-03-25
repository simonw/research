# Scramjet Web Proxy Research Notes

## Research Process

### Sources Consulted
- GitHub repo: MercuryWorkshop/scramjet (367 stars, 578 forks as of March 2026)
- Titanium Network documentation: docs.titaniumnetwork.org/proxies/scramjet/
- Titanium Network rewriting guide: docs.titaniumnetwork.org/guides/interception-proxy-guide/rewriting
- NPM package: @mercuryworkshop/scramjet
- BrightCoding blog post (March 2026)
- GitHub issues page (21 open issues)
- Mercury Workshop organization page on TN docs
- Multiple web searches for technical details, community info, and comparisons

### Key Findings

1. **Scramjet is the official successor to Ultraviolet** - Ultraviolet's own GitHub description says "Succeeded by Scramjet"
2. **Rust-to-WASM rewriter is the core innovation** - compiled from Rust using wasm-bindgen, optimized with wasm-opt
3. **Uses "Byte Span Rewrite" approach** for JS rewriting - trades AST manipulation capability for speed
4. **Mercury Workshop split from Titanium Network in Summer 2025** - now independent but Scramjet is still maintained for the community
5. **Last commit to main branch was January 16, 2025** according to GitHub page fetch, but CI builds continue (latest pre-release Feb 16, 2026) - suggests active development may be on a v2 branch
6. **WebSocket tests were added Dec 30, 2025** on the v2 branch, indicating ongoing development
7. **21 open issues** - majority are site-specific compatibility problems
8. **33,000+ Discord members** in the Titanium Network community
9. **200+ projects** depend on the scramjet npm package
10. **AGPL-3.0 license** - copyleft, requires source disclosure for network use

### Interesting Technical Details
- CSS rewriting uses simple RegExp (only needs to handle @import statements)
- JS rewriting is the hardest challenge - location objects are non-configurable
- Scramjet uses Byte Span Rewrites for speed but can't support AST plugins
- Function.prototype.toString is a known challenge (reveals rewritten code)
- CAPTCHA handling works by preserving the full request chain and session context
- Claims "sub-millisecond processing times" and 2-3x speedup over htmlparser2
- Dev server runs on port 1337 with hot reload

### Gaps in Research
- Could not access NPM package page (403 error) for download stats
- GitHub releases page had loading errors
- No formal benchmarks published; performance claims are from documentation/blog posts
- Limited information about internal iframe handling specifics
