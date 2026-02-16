# Untimely: Pivoting to Random Scheduling as a Service

## Research and Feasibility Analysis

---

## 1. Executive Summary

[Untimely](https://untimely.app) is a consumer-facing application that schedules randomly recurring, spontaneous events. Today, it handles both the **scheduling** (deciding *when* something happens) and the **compute** (sending SMS, emails, Slack messages, and invoking LLMs). The proposed pivot strips all compute and repositions Untimely as a pure **random scheduling API layer** — when a schedule fires, it calls a developer-configured webhook. The developer decides what happens next.

The strategic rationale is straightforward: **no existing scheduling service offers stochastic timing.** Every competitor — cron-job.org, Cronhub, EasyCron, Google Cloud Scheduler, Cronhooks, HookPulse, Posthook — is strictly deterministic. They fire at exact times specified by cron expressions or Unix timestamps. The concept of "fire somewhere between 9am and 5pm, roughly twice per day, at random times" does not exist on any platform. This is a genuine market gap.

The second strategic bet is **AI-native design**. By serving `/llms.txt` at the root, building an MCP (Model Context Protocol) server, and making the homepage markdown-parseable, Untimely becomes the scheduling service that AI agents can discover and integrate with autonomously. A developer (or their coding agent) should be able to say "schedule this task randomly using untimely.app" and have it work — the agent reads the homepage, finds the API docs, generates the integration code, and moves on. No scheduling service offers this today.

This document is a critical assessment, not a pitch deck. The core question: **is this gap worth filling, or does it exist because nobody needs it filled?**

---

## 2. Competitive Landscape

### Comparison Table

| Service | Type | Random Scheduling | API Access | Free Tier | Webhook-Native | Auth Model |
|---------|------|:-----------------:|:----------:|:---------:|:--------------:|------------|
| **cron-job.org** | Cron SaaS | No | REST API | Unlimited (donation-funded) | Yes | Bearer token |
| **Cronhub** | Monitoring + Cron | No | REST API | Limited | Yes | API key |
| **Crontap** | Cron Builder | No | No | Yes | No | N/A |
| **EasyCron** | Cron SaaS | No | REST API | 1 job | Yes | API key |
| **Google Cloud Scheduler** | Enterprise Cron | No | gcloud CLI + REST | 3 jobs free | HTTP/Pub-Sub | IAM/OAuth |
| **Posthook** | Task Scheduling API | No | REST API | 500 requests | Yes | API key |
| **Cronhooks** | Webhook Scheduler | No | REST API | 5 schedules | Yes (primary) | API key + HMAC signing |
| **HookPulse** | Webhook Scheduler | No | REST API | None | Yes (primary) | API key |
| **Untimely (proposed)** | Random Scheduling API | **Yes** | REST API + MCP | Proposed | Yes (primary) | API key + HMAC signing |

### Analysis

Every competitor in this space is fundamentally deterministic. They accept cron expressions (`0 9 * * MON-FRI`) or precise timestamps (`2024-03-15T14:30:00Z`) and fire at exactly those times. There is no variance, no randomness, no "roughly around this time."

The closest workaround a developer has today is to build random scheduling on top of a deterministic service: use their own code to pick a random time, then create a one-shot job on cron-job.org or Posthook for that time, then repeat the process for the next occurrence. This is fragile — what if the random time is already in the past? What about timezone transitions? What about managing the lifecycle of thousands of ephemeral one-shot jobs? What about ensuring random times are reasonably spread rather than clustered? This is real engineering work that developers would rather not do.

**Market sizing reality check.** The scheduling SaaS market is small. cron-job.org is entirely donation-funded and has been since 2008. HookPulse's pricing starts at $5.88/month. Cronhooks offers 5 free schedules. These are not venture-scale businesses. Untimely should enter this market with realistic expectations about revenue potential.

---

## 3. The Unique Value Proposition

### What "Random Scheduling" Actually Means

Given a **frequency** (e.g., "3 times per week") and an optional **time window** (e.g., "9am–5pm EST, weekdays only"), the system:

1. Computes random fire times within each upcoming period
2. Ensures reasonable spread (not three fires clustered in 10 minutes)
3. Handles timezone transitions (DST changes, user timezone shifts)
4. Persists the computed times so they're auditable and predictable to the system (but not to the user)
5. Fires webhooks at those times with retry logic and delivery confirmation

The developer does not know when the call will come. They just know it will come roughly N times per period, within the specified window.

### Why This Is Hard to DIY

On the surface, random scheduling looks trivial — `Math.random() * windowSize`. In practice, it requires:

- **Timezone math**: IANA timezone handling with DST transitions. A "9am–5pm" window in `America/New_York` shifts by an hour twice per year. Edge cases are brutal.
- **Spread distribution**: Naive random selection can cluster multiple fires within minutes. Good random scheduling uses minimum-gap constraints or Poisson process modeling.
- **Missed window handling**: If a period passes without all scheduled fires completing (server downtime, queue backup), does the system catch up or skip? Both behaviors are valid; the developer needs to choose.
- **Retry logic**: If a webhook fails, the system needs exponential backoff without drifting the fire time outside the configured window.
- **Audit trails**: For compliance use cases (random drug testing, random safety inspections), the system must prove that timing was genuinely random and not manipulated.
- **Persistence**: Random fire times must be computed ahead of time and stored, not generated on-the-fly, so the system is deterministic internally even though it appears random externally.

### Positioning

> *Untimely is the scheduling API for when the timing should be unpredictable. Define a frequency and a window; we handle the randomness, the retries, and the delivery.*

---

## 4. Proposed API Design

### Authentication

API key-based auth via the `Authorization` header:

```
Authorization: Bearer utly_sk_live_abc123def456
```

Key design decisions:
- **Prefixed keys**: `utly_sk_live_` for production, `utly_sk_test_` for sandbox. This makes keys greppable in codebases and distinguishable at a glance.
- **One-click signup**: Email + password → instant API key on the dashboard. No sales calls, no approval process.
- **Key rotation**: Dashboard supports creating new keys and revoking old ones with a grace period.
- **Rate limiting**: Per-key limits (e.g., 100 API requests/minute, configurable per tier).
- **HMAC signing on outbound webhooks**: Each schedule gets a `signing_secret`. Outbound webhooks include an `Untimely-Signature` header containing `t=<timestamp>,v1=<hmac_sha256>`, following Stripe's well-documented pattern.

### Core Data Model

```
Schedule {
  id: string                    // ULID or UUID
  name: string                  // Human-readable label
  description: string           // Optional description

  // Frequency definition
  frequency: {
    type: "daily" | "weekly" | "monthly"
    count: number               // e.g., 3 means "3 times per <type>"
  }

  // Time window constraint
  window: {
    start_time: string          // "09:00" (HH:MM, 24h)
    end_time: string            // "17:00"
    timezone: string            // IANA timezone, e.g., "America/New_York"
    days_of_week?: number[]     // 0=Mon, 6=Sun. Optional filter.
  }

  // Delivery target
  target: {
    type: "webhook"
    url: string                 // The URL to POST to
    method?: "POST" | "GET" | "PUT"   // Default: POST
    headers?: Record<string, string>  // Custom headers
    body?: object               // Custom JSON payload
  }

  // State
  enabled: boolean
  signing_secret: string        // For HMAC verification (read-only)
  created_at: string            // ISO 8601
  updated_at: string            // ISO 8601
  next_fire_at: string | null   // Read-only: next computed fire time
  last_fired_at: string | null  // Read-only: last successful fire
}
```

```
Execution {
  id: string
  schedule_id: string
  fired_at: string              // ISO 8601
  status: "pending" | "delivered" | "failed" | "retrying"
  response_code: number | null
  response_body: string | null  // Truncated to 1KB
  retry_count: number
  next_retry_at: string | null
}
```

### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/schedules` | Create a new schedule |
| `GET` | `/v1/schedules` | List all schedules (paginated) |
| `GET` | `/v1/schedules/:id` | Get schedule details |
| `PATCH` | `/v1/schedules/:id` | Update a schedule |
| `DELETE` | `/v1/schedules/:id` | Delete a schedule |
| `POST` | `/v1/schedules/:id/pause` | Pause a schedule |
| `POST` | `/v1/schedules/:id/resume` | Resume a schedule |
| `POST` | `/v1/schedules/:id/trigger` | Manually trigger now (for testing) |
| `GET` | `/v1/schedules/:id/executions` | List execution history (paginated) |
| `GET` | `/v1/executions/:id` | Get execution details |
| `GET` | `/v1/account` | Account info and usage stats |

### Example: Creating a Schedule

```bash
curl -X POST https://api.untimely.app/v1/schedules \
  -H "Authorization: Bearer utly_sk_live_abc123" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Random team kudos",
    "frequency": { "type": "weekly", "count": 3 },
    "window": {
      "start_time": "10:00",
      "end_time": "16:00",
      "timezone": "America/New_York",
      "days_of_week": [0, 1, 2, 3, 4]
    },
    "target": {
      "type": "webhook",
      "url": "https://myapp.com/api/kudos-trigger",
      "method": "POST",
      "body": { "action": "send_random_kudos" }
    }
  }'
```

**Response:**
```json
{
  "id": "sched_01HYX3K9M7...",
  "name": "Random team kudos",
  "enabled": true,
  "signing_secret": "whsec_abc123...",
  "next_fire_at": "2026-02-17T11:23:00-05:00",
  "created_at": "2026-02-16T20:00:00Z"
}
```

### Webhook Delivery Specification

When a schedule fires, Untimely POSTs to the configured URL:

```http
POST /api/kudos-trigger HTTP/1.1
Host: myapp.com
Content-Type: application/json
Untimely-Signature: t=1708120980,v1=5257a869e7ecebeda32affa62cdca3fa51cad7e77a0e56ff536d0ce8e108d8bd
Untimely-Execution-Id: exec_01HYX3M2N8...
Untimely-Schedule-Id: sched_01HYX3K9M7...

{
  "action": "send_random_kudos"
}
```

- **Retry policy**: 3 retries with exponential backoff (30 seconds, 5 minutes, 30 minutes)
- **Timeout**: 30 seconds per attempt
- **Success**: Any 2xx response code
- **Failure logging**: Response code and truncated body stored in the Execution record

---

## 5. AI-Native Features

### 5a. `/llms.txt` and `/llms-full.txt`

The [llms.txt convention](https://llmstxt.org) provides a standardized way for websites to expose structured information to LLMs. Over 844,000 websites have adopted it, including Stripe, Anthropic, and Cloudflare. Untimely should serve these files at its root.

**Draft `/llms.txt`:**

```markdown
# Untimely API

> Random scheduling as a service. Define a frequency and a time window; we call your webhook at unpredictable times within that window.

## Docs

- [Quickstart](https://untimely.app/docs/quickstart): Get your API key and create your first random schedule in under 2 minutes
- [API Reference](https://untimely.app/docs/api): Full REST API documentation with examples
- [Authentication](https://untimely.app/docs/auth): API key setup, rotation, and webhook signature verification
- [Webhook Verification](https://untimely.app/docs/webhooks): How to verify HMAC-SHA256 signatures on incoming webhooks
- [MCP Server](https://untimely.app/docs/mcp): Connect AI agents to Untimely via the Model Context Protocol

## Optional

- [Rate Limits](https://untimely.app/docs/rate-limits): Request and execution limits by tier
- [Retry Policy](https://untimely.app/docs/retries): Webhook retry behavior and dead-letter handling
- [Pricing](https://untimely.app/pricing): Free and paid tier details
```

**`/llms-full.txt`** would be a complete markdown export of all API documentation — endpoint descriptions, data models, request/response examples, error codes, and integration guides. Target size: under 100K tokens so models can ingest it in a single context window.

### 5b. Markdown-Readable Homepage

The homepage at untimely.app should be designed so that an AI agent can fetch it, parse the HTML-to-markdown conversion, and understand how to integrate. This means:

- Clean semantic HTML that degrades to readable markdown
- A visible **"For AI Agents"** section containing:
  - API base URL: `https://api.untimely.app/v1`
  - Auth method: `Authorization: Bearer <your_api_key>`
  - Link to `/llms.txt`
  - Link to MCP server endpoint
- Copy-paste curl examples directly on the homepage
- `<meta>` tags with structured descriptions of the API's capabilities

The goal: an agent reading the homepage should be able to generate a working integration without consulting external documentation.

### 5c. MCP Server

An MCP (Model Context Protocol) server lets AI agents interact with Untimely directly through their tooling — Claude Desktop, VS Code with Copilot, Claude Code, or any MCP-compatible host.

**Transport**: Streamable HTTP (remote MCP server hosted by Untimely)

**Tools to expose:**

| Tool | Description |
|------|-------------|
| `create_schedule` | Create a new random schedule with frequency, window, and webhook target |
| `list_schedules` | List all schedules for the authenticated account |
| `get_schedule` | Get details and next fire time for a specific schedule |
| `update_schedule` | Modify an existing schedule's frequency, window, or target |
| `delete_schedule` | Permanently remove a schedule |
| `pause_schedule` | Pause a schedule without deleting it |
| `resume_schedule` | Resume a paused schedule |
| `trigger_schedule` | Manually fire a schedule now (for testing) |
| `list_executions` | View delivery history for a schedule |

**Resources to expose:**

| Resource URI | Description |
|-------------|-------------|
| `untimely://docs/quickstart` | Quickstart guide |
| `untimely://docs/api-reference` | Full API reference |
| `untimely://account/usage` | Current usage stats and limits |

**Prompts to expose:**

| Prompt | Description |
|--------|-------------|
| `create-random-schedule` | Guided template: walks the agent through defining frequency, window, timezone, and webhook target |

**Auth**: API key passed during MCP connection setup via Bearer token header.

This means a developer can tell their AI assistant: *"Create a random schedule that pings my Slack channel 2-3 times per day during business hours"* — and the agent handles it end-to-end through the MCP server without the developer touching the API directly.

---

## 6. Integration Surfaces

### 6a. Webhooks (Primary Integration)

The foundational integration. Every schedule fires a webhook — all other integrations are built on top of or alongside this.

- Generic HTTP POST/GET/PUT to any URL
- HMAC-SHA256 signing via `Untimely-Signature` header (Stripe's pattern — well-documented, battle-tested, widely understood)
- Retry with exponential backoff: 30s → 5min → 30min, up to 3 attempts
- Delivery logs: response code, truncated body, timing, retry count
- Manual retry from dashboard or API
- Dead-letter behavior: after 3 failures, execution marked as `failed`, visible in execution history

### 6b. Slack

Two levels of integration:

1. **Basic (webhook-native)**: Developer creates a Slack Incoming Webhook URL and sets it as the schedule's target. The `body` field uses Slack Block Kit JSON for rich formatting. This works today with zero additional infrastructure.
2. **Future (Slack App)**: A first-party Untimely Slack app with a `/untimely` slash command for creating and managing schedules directly from Slack. This is a convenience layer that increases surface area but requires maintaining a Slack app.

Recommendation: launch with webhook-native Slack support only. Build the Slack App once there's demand.

### 6c. Discord

Same pattern as Slack: POST to a Discord webhook URL with rich embed formatting in the body. Discord's webhook format is simpler than Slack's Block Kit, and the integration is trivially supported by the webhook-native architecture.

### 6d. Email

This is a design tension point. If Untimely wants to be "pure scheduling," it should just POST to the developer's endpoint and let them send the email. But offering a basic email target lowers the barrier for non-developer users who just want random email reminders.

Recommendation: support a simple `"type": "email"` target that sends a basic email via a transactional service (Postmark, SendGrid). Keep it minimal — plain text subject + body, no templates. This preserves the consumer use case without overcomplicating the API.

### 6e. Automation Platforms (Zapier, Make, n8n)

Because Untimely fires webhooks, it is already compatible with any platform that can receive webhooks:

- **Zapier**: "Webhooks by Zapier" trigger catches Untimely's POST. A dedicated Untimely Zapier app would provide a polished UX (dropdown to select schedule, auto-authentication) but is not required for launch.
- **Make (Integromat)**: Custom webhook module receives Untimely's POST directly.
- **n8n**: Webhook trigger node works out of the box.

The dedicated Zapier/Make apps are convenience layers that reduce friction for non-developer users. They should be built post-launch when usage patterns are clearer.

---

## 7. Use Cases with Critical Viability Assessment

| Use Case | How It Works with Untimely | Viability | Assessment |
|----------|---------------------------|:---------:|------------|
| **Chaos engineering** | Random schedule triggers fault injection (Netflix Chaos Monkey pattern). Teams want unpredictable timing by definition. Currently they build custom schedulers for this. | **Strong** | Genuine technical need. Unpredictability is the core requirement, not a nice-to-have. This is probably the single strongest developer use case. |
| **Team appreciation / kudos** | Schedule fires webhook to Slack bot that picks a random team member and sends praise. | **Strong** | Natural fit. The randomness prevents "appreciation fatigue" that comes with predictable cadences. Companies like Bonusly validate this market. |
| **Wellness reminders** | Random reminders to stretch, hydrate, take a walk — at varying times so they don't become background noise. | **Strong** | Classic Untimely use case, proven by the existing consumer product. A reminder at a random time is harder to ignore than one at 2pm every day. |
| **Gamification / random rewards** | Random reward drops in apps. Variable-ratio reinforcement schedules (slot machine psychology). | **Strong** | Well-studied in behavioral psychology. Random rewards drive more engagement than predictable ones. Clean API use case — the app calls Untimely to schedule the next reward drop. |
| **Security testing** | Random scheduling of penetration tests or vulnerability scans. | **Moderate** | Real need, but security teams tend to use specialized platforms. Untimely would be the trigger, not the scanner. Useful as a component, not a complete solution. |
| **Load testing** | Random traffic spikes to test autoscaling behavior. | **Moderate** | Useful but niche. Most load testing tools (k6, Locust) have their own scheduling. Untimely adds the randomness layer on top. |
| **Compliance auditing** | Random audit scheduling for regulatory requirements (random drug testing, safety inspections). | **Moderate** | Real regulatory requirement in some industries. But compliance teams use specialized software with audit trails and certification. Untimely would need compliance-grade logging to compete. |
| **Content publishing** | Post to social media at random-seeming times for a more "organic" feel. | **Moderate** | Social media managers generally want *optimized* posting times, not random ones. Some want "varied" times, which is subtly different from random. Moderate fit. |
| **Employee spot checks** | Random quality assurance sampling or check-ins. | **Moderate** | Real workplace need, but sensitive territory (surveillance concerns). The tool works; the social dynamics are complex. |
| **Spaced repetition / learning** | Randomized study reminders for language learning, skill building. | **Weak** | Spaced repetition has its own well-researched algorithms (SM-2, FSRS) that are specifically *not* random — they're optimized for memory retention. Truly random timing contradicts the science. |
| **A/B testing timing** | Run experiments at random intervals. | **Weak** | A/B testing platforms handle their own experiment timing and statistical rigor. Hard to see why you'd outsource just the scheduling component. |

### Summary

The strongest use cases share a common trait: **unpredictability is the core requirement, not an optimization.** Chaos engineering *needs* randomness. Wellness reminders *benefit* from randomness. Gamification *leverages* randomness. The weaker use cases are those where randomness is tangential to the actual problem (A/B testing, spaced repetition) or where specialized platforms already handle the full workflow (compliance, security).

---

## 8. Technical Considerations for the Pivot

### What to Strip from the Current Codebase

- **SMS sending** (Twilio integration or equivalent) — remove entirely
- **LLM invocation** — remove entirely
- **Direct email composition** — replace with simple webhook-to-email-service bridge or keep as a thin "email target" option
- **Direct Slack message composing** — replace with generic webhook POST to Slack webhook URLs

### What to Build

| Component | Priority | Description |
|-----------|:--------:|-------------|
| REST API layer | **P0** | Versioned `/v1/` API with the endpoints described in Section 4 |
| API key management | **P0** | Generation, rotation, revocation, rate limiting |
| Webhook delivery engine | **P0** | HMAC signing, retry queue, delivery logging |
| Developer dashboard | **P0** | Signup, API key management, schedule management, execution logs |
| `/llms.txt` + `/llms-full.txt` | **P1** | Static files served at root, updated alongside docs |
| API documentation | **P1** | Interactive docs (Mintlify or similar — Mintlify has first-class llms.txt support) |
| MCP server | **P1** | Streamable HTTP transport, tools for all schedule operations |
| Zapier integration | **P2** | Dedicated Zapier trigger app |
| Slack app | **P2** | `/untimely` slash command for in-Slack schedule management |

### Infrastructure

- **Database**: PostgreSQL. Stores schedules, pre-computed fire times, execution logs, API keys, and account data.
- **Scheduling worker**: A cron-like process that runs every minute, checks for schedules whose next fire time is now or in the past, and enqueues webhook delivery jobs. For random scheduling, this worker also computes random fire times for each upcoming period and persists them.
- **Webhook delivery queue**: SQS, Redis + BullMQ, or similar. Separate from the scheduling worker to ensure delivery retries don't block schedule evaluation.
- **Web framework**: The current stack is Next.js. The API could be Next.js API routes, but for a developer API platform, a separate lightweight service (Hono on Cloudflare Workers, or Fastify on a VPS) may be cleaner and cheaper to run.

### Pricing Model

| Tier | Price | Schedules | Executions/Month | Features |
|------|------:|:---------:|:-----------------:|----------|
| **Free** | $0 | 5 | 1,000 | 1 API key, 3-day execution logs |
| **Developer** | $9/mo | 50 | 25,000 | 3 API keys, 30-day logs, HMAC signing |
| **Team** | $29/mo | 500 | 100,000 | Unlimited keys, 90-day logs, priority delivery |
| **Enterprise** | Custom | Custom | Custom | SLA, dedicated support, SSO |

**Reality check**: cron-job.org is free. Cronhooks charges from $0. The willingness-to-pay premium must come from (a) the randomness feature itself, (b) reliability and developer experience, and (c) the AI-native integration story. The free tier is non-negotiable for adoption.

---

## 9. Honest Assessment: Will This Service Be Useful?

### Arguments For

1. **The market gap is real and confirmed.** A thorough survey of every major scheduling service reveals zero that offer random/stochastic timing. A web search for "random scheduling API" returns academic papers on stochastic scheduling theory, not SaaS products. This gap is genuine.

2. **The use cases are genuine.** Chaos engineering teams, wellness app developers, gamification systems, and team culture tools all need unpredictable timing. These are not hypothetical — Netflix built Chaos Monkey specifically because they needed random fault injection, and every company practicing chaos engineering reinvents this wheel.

3. **AI-native design is a timing advantage.** No scheduling service has MCP support or serves `/llms.txt`. As AI-assisted development becomes mainstream, being the first scheduling service that agents already know how to use could be a compounding advantage. When an agent encounters "schedule this randomly," Untimely should be the answer it finds.

4. **The API is simple.** A developer can go from zero to a working random schedule in under 2 minutes: sign up, get API key, make one curl request. Low friction means low barrier to trial.

5. **The existing codebase has the hardest part.** The random scheduling engine — with its timezone handling, spread distribution, and missed-window logic — already exists in Untimely. The pivot is about removing code and exposing an API, not building core scheduling logic from scratch.

### Arguments Against

1. **The market may be small because the need is small.** Most developers who need "random-ish" timing can write `setTimeout(callback, Math.random() * window)` and call it done. The value of outsourcing this to a third-party service — with its own reliability concerns and vendor dependency — is not self-evident. Untimely must convince developers that the edge cases (timezone math, spread distribution, retry logic, audit trails) are worth paying someone else to handle.

2. **Scheduling services are low-margin businesses.** cron-job.org has survived on donations since 2008. HookPulse starts at $5.88/month. Cronhooks offers a free tier with 5 schedules. These are not markets where customers pay a lot. Untimely's pricing power comes from a feature (randomness) that developers may not value enough to pay for.

3. **The pivot changes the audience.** Untimely today serves consumers who want random reminders via SMS and email. The pivoted product serves developers who want a scheduling API. These are fundamentally different audiences requiring different go-to-market strategies, different documentation standards, different support expectations, and different reliability guarantees. The pivot is not just a code change — it's a business model change.

4. **Reliability expectations are high for infrastructure.** When a developer integrates Untimely as their scheduling layer, a missed webhook means their downstream system fails silently. Building and maintaining the trust required to be someone's scheduling infrastructure takes time, uptime track record, and transparent incident communication. This is table stakes, not a differentiator.

5. **The AI-native angle is a bet on the future.** Today, very few developers are building with MCP. The `/llms.txt` convention is growing rapidly (844K+ sites) but no major AI platform has officially committed to reading these files as a primary integration mechanism. The payoff from AI-native design may take 12–24 months to materialize. It is the right bet, but it requires patience.

### Verdict

The random scheduling gap is genuine, and the strongest use cases — chaos engineering, gamification, wellness, team culture — represent real developer needs where unpredictability is a core requirement. However, the total addressable market is small, and willingness-to-pay is uncertain.

**Untimely is viable as a focused niche product** — a profitable small SaaS or a meaningful open-source project with a hosted tier. It is **unlikely to be a venture-scale business** given the market dynamics of scheduling services.

The **AI-native design is the most interesting strategic bet.** If agent-driven development becomes mainstream in the next 1–2 years, being the scheduling service that agents already know how to use could be a significant and compounding advantage. The moat is not the random scheduling algorithm — it's being the default answer when an agent encounters the need.

**Recommendation**: Pursue the pivot with modest revenue expectations. Prioritize developer experience above all else. Keep infrastructure costs low (serverless where possible). Ship the free tier, `/llms.txt`, and MCP server early. Validate willingness-to-pay with the Developer tier before investing in Team/Enterprise features. Measure success by adoption rate and agent integration frequency, not just revenue.

---

## Appendix A: Draft `/llms.txt` for untimely.app

```markdown
# Untimely API

> Random scheduling as a service. Define a frequency and time window;
> we call your webhook at unpredictable times within that window.
> No other scheduling service offers stochastic timing.

## Getting Started

- [Quickstart](https://untimely.app/docs/quickstart): Sign up, get an API key, create your first random schedule in under 2 minutes
- [API Reference](https://untimely.app/docs/api): Complete REST API documentation
- [Authentication](https://untimely.app/docs/auth): API keys, rate limits, and webhook signature verification

## Key Concepts

- [Random Scheduling](https://untimely.app/docs/concepts/random-scheduling): How frequency, time windows, and randomness work together
- [Webhook Delivery](https://untimely.app/docs/webhooks): HMAC-SHA256 verification, retry policy, delivery logs
- [MCP Server](https://untimely.app/docs/mcp): Connect AI agents directly to Untimely

## Optional

- [Pricing](https://untimely.app/pricing): Free tier includes 5 schedules and 1,000 executions/month
- [Rate Limits](https://untimely.app/docs/rate-limits): 100 API requests/minute on free tier
- [Changelog](https://untimely.app/changelog): API versioning and breaking changes
```

## Appendix B: MCP Server Tool Definitions

```json
{
  "tools": [
    {
      "name": "create_schedule",
      "description": "Create a new randomly-timed schedule. Specify how often it should fire (frequency), when it's allowed to fire (window), and where to send the webhook (target).",
      "inputSchema": {
        "type": "object",
        "required": ["name", "frequency", "window", "target"],
        "properties": {
          "name": { "type": "string", "description": "Human-readable name for this schedule" },
          "frequency": {
            "type": "object",
            "required": ["type", "count"],
            "properties": {
              "type": { "type": "string", "enum": ["daily", "weekly", "monthly"] },
              "count": { "type": "integer", "minimum": 1, "description": "How many times per period" }
            }
          },
          "window": {
            "type": "object",
            "required": ["start_time", "end_time", "timezone"],
            "properties": {
              "start_time": { "type": "string", "description": "HH:MM format, e.g., '09:00'" },
              "end_time": { "type": "string", "description": "HH:MM format, e.g., '17:00'" },
              "timezone": { "type": "string", "description": "IANA timezone, e.g., 'America/New_York'" },
              "days_of_week": { "type": "array", "items": { "type": "integer", "minimum": 0, "maximum": 6 }, "description": "0=Mon through 6=Sun" }
            }
          },
          "target": {
            "type": "object",
            "required": ["url"],
            "properties": {
              "url": { "type": "string", "description": "Webhook URL to POST to when the schedule fires" },
              "method": { "type": "string", "enum": ["POST", "GET", "PUT"], "default": "POST" },
              "headers": { "type": "object", "description": "Custom headers to include" },
              "body": { "type": "object", "description": "Custom JSON payload" }
            }
          }
        }
      }
    },
    {
      "name": "list_schedules",
      "description": "List all schedules for the authenticated account. Returns schedule names, IDs, enabled status, and next fire times.",
      "inputSchema": { "type": "object", "properties": {} }
    },
    {
      "name": "get_schedule",
      "description": "Get full details of a specific schedule including its next computed fire time.",
      "inputSchema": {
        "type": "object",
        "required": ["schedule_id"],
        "properties": {
          "schedule_id": { "type": "string", "description": "The schedule ID (e.g., sched_01HYX3K9M7...)" }
        }
      }
    },
    {
      "name": "trigger_schedule",
      "description": "Manually fire a schedule right now, bypassing the random timing. Useful for testing your webhook integration.",
      "inputSchema": {
        "type": "object",
        "required": ["schedule_id"],
        "properties": {
          "schedule_id": { "type": "string", "description": "The schedule ID to trigger" }
        }
      }
    }
  ]
}
```

## Appendix C: Webhook Payload Specification

When a schedule fires, Untimely delivers an HTTP request to the configured target URL:

**Headers:**
```
Content-Type: application/json
Untimely-Signature: t=1708120980,v1=5257a869...
Untimely-Execution-Id: exec_01HYX3M2N8...
Untimely-Schedule-Id: sched_01HYX3K9M7...
Untimely-Schedule-Name: Random team kudos
```

**Body:** The exact JSON object specified in the schedule's `target.body` field. If no body was specified:

```json
{
  "schedule_id": "sched_01HYX3K9M7...",
  "schedule_name": "Random team kudos",
  "execution_id": "exec_01HYX3M2N8...",
  "fired_at": "2026-02-17T11:23:00-05:00"
}
```

**Signature verification (Node.js example):**
```javascript
const crypto = require('crypto');

function verifySignature(payload, header, secret) {
  const [tPart, vPart] = header.split(',');
  const timestamp = tPart.split('=')[1];
  const signature = vPart.split('=')[1];
  const expected = crypto
    .createHmac('sha256', secret)
    .update(`${timestamp}.${JSON.stringify(payload)}`)
    .digest('hex');
  return crypto.timingSafeEqual(
    Buffer.from(signature), Buffer.from(expected)
  );
}
```
