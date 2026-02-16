# Research Notes: Random Scheduling Service Pivot

## Research Process

### Phase 1: Understanding the Current Product
- Fetched untimely.app homepage to understand current features
- Untimely is a consumer-facing app for scheduling randomly recurring, spontaneous events
- Users define events with customizable frequencies (daily, weekly, monthly, yearly) and optional time windows
- Notification channels: Email, SMS, Slack, Zapier
- Use cases shown on homepage: team appreciation, wellness reminders, social connection prompts
- Built with Next.js
- GitHub repo at github.com/Kahtaf/untimely (returned 404 — likely private)

### Phase 2: Competitive Landscape Research
Researched every major scheduling service to determine if any offer random/stochastic scheduling:

1. **cron-job.org** — Free, donation-funded. REST API with bearer token auth. Job model with schedule arrays for hours/minutes/days/months. Up to 60 executions/hour. Entirely deterministic. No random timing.
2. **Cronhub** — Developer-focused monitoring + scheduling. No random.
3. **Crontap** — Human-readable cron syntax builder. No API focus. No random.
4. **EasyCron** — Simple web-based cron with execution logs. No random.
5. **Google Cloud Scheduler** — Enterprise-grade. Cron expressions + HTTP/Pub-Sub/App Engine targets. No random.
6. **Posthook** — Task scheduling API. One-time or sequenced HTTP callbacks at precise times. API key auth. 500 free scheduled requests. No random.
7. **Cronhooks** — Webhook-native scheduling. Cron expressions or one-time timestamps. Free tier: 5 schedules. Security signatures for webhook verification. No random.
8. **HookPulse** — Elixir/OTP-based webhook scheduler. Cron, interval, clocked, and solar event scheduling. Usage-based pricing from $5.88/month. FIFO queues, idempotency keys, retry logic. No random.

**Key finding: Zero competitors offer random/stochastic scheduling. Every single one is deterministic.**

### Phase 3: AI-Native Design Research
- Researched /llms.txt convention: adopted by 844,000+ websites including Stripe, Anthropic, Cloudflare
- Structure: H1 title, blockquote description, sections with links, optional advanced topics
- Companion /llms-full.txt for complete documentation export
- MCP (Model Context Protocol): universal standard for AI-to-service communication
- Adopted by both OpenAI and Anthropic
- Supports tools, resources, and prompts as primitives
- Streamable HTTP transport for remote servers
- TypeScript and Python SDKs available at modelcontextprotocol.io

### Phase 4: Integration Patterns Research
- Webhook best practices: HMAC-SHA256 signing (Stripe pattern), retry with exponential backoff, delivery status logging
- Slack: incoming webhooks with Block Kit formatting
- Discord: webhook URLs with rich embed support
- Zapier: REST hooks (polling or webhook-based triggers)
- Make/n8n: generic webhook trigger compatibility

### Key Insights Discovered
1. The market gap is genuinely unoccupied — nobody does random scheduling as a service
2. The scheduling SaaS market is small and low-margin (donation-funded to ~$9/mo)
3. The strongest use cases are where unpredictability is a core requirement, not a nice-to-have
4. AI-native design (/llms.txt + MCP) is a genuine timing advantage — no scheduling service has this yet
5. The biggest risk is that random scheduling is easy enough to DIY that developers won't pay for it
6. cron-job.org's API design (RESTful, bearer auth, simple job model) is the right pattern to follow
