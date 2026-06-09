# Neuron Platform: Full Implementation Plan
**Two Products. One Platform. One Vision.**

> *"The memory layer that sits between humans and AI — at the code level and at the soul level."*

---

## Product A: Neuron Core™ (Enterprise Edition)
**What it is:** The existing codebase memory engine, hardened for enterprise deployment.

---

### Enterprise Feature Gap Analysis
What we have vs. what enterprises need:

| Feature | Current State | Enterprise Requirement |
|---|---|---|
| Local SQLite Indexing | ✅ Done | ✅ Good |
| MCP Tool Exposure | ✅ Done | ✅ Good |
| Credential Sanitization | ✅ Done | ✅ Good |
| Multi-Project Support | ✅ Done | ✅ Good |
| **Commercial License** | ❌ Missing | 🔴 Critical |
| **Audit Logging** | ❌ Missing | 🔴 Critical |
| **Team/Org Mode** | ❌ Missing | 🔴 Critical |
| **Admin Policy Controls** | ❌ Missing | 🔴 Critical |
| **SSO / Identity Integration** | ❌ Missing | 🟡 Important |
| **CI/CD Pipeline Mode** | ❌ Missing | 🟡 Important |

---

### Phase A.1 — Commercial License Gate (Week 1)
**Goal:** Make Neuron Core legally sellable to enterprises TODAY.

#### Steps:
1. **Add Dual-License Structure to Repository**
   - Keep AGPLv3 for open-source community use (code is free to inspect).
   - Add `LICENSE-COMMERCIAL.md` to the repository stating that embedding Neuron Core in a closed-source product requires a commercial license.
   - Add this to `README.md`: *"For commercial embedding licenses, contact enterprise@ai-neuron.org"*

2. **Set Up Enterprise Contact Infrastructure**
   - Create `enterprise@ai-neuron.org` forwarding alias (via Amazon SES or Cloudflare Email Routing, both free).
   - Add a dedicated "Enterprise" section to the landing page at `ai-neuron.org` with a contact form.

3. **Set License Pricing (Starting Point)**
   - **Team License:** \$199/month (up to 25 developer seats)
   - **Org License:** \$499/month (up to 100 developer seats)
   - **Unlimited/OEM License:** Custom (for companies that want to embed Neuron Core in their own product)

---

### Phase A.2 — Audit Logging Engine (Week 2-3)
**Goal:** Every context query an AI agent makes is logged, timestamped, and exportable.

#### Architecture:
- A new Rust module `audit.rs` appends a structured JSON line to `~/.neuron/audit.log` on every MCP tool invocation.
- Each log line records:
  - **Tool called** (e.g., `get_file_content`)
  - **Query parameters** (what symbol/file was requested)
  - **Timestamp + session ID**
  - **Response size in bytes** (not the content — just metadata)
- An `audit export` CLI command generates a signed, tamper-evident audit report (JSON or CSV).

#### Why enterprises need this:
- **SOC 2 Compliance:** Auditors require logs of all data access by AI tools.
- **GDPR / Data Residency:** Enterprises need to prove what data their AI systems accessed and when.

---

### Phase A.3 — Team/Org Mode (Week 3-4)
**Goal:** Allow a team of developers to share a single Neuron index for a shared codebase.

#### Architecture:
- A `neuron serve` command starts a lightweight HTTP/WebSocket server exposing the same MCP JSON-RPC interface over the local network (instead of stdio).
- All team members' AI agents point their MCP config to `http://neuron-server.local:7070` instead of the local binary.
- The shared index runs on one machine (e.g., a dev server or a senior engineer's workstation) and all agents benefit from a single, continuously updated index.

---

### Phase A.4 — CI/CD Pipeline Mode (Week 4-5)
**Goal:** Neuron can run inside GitHub Actions / GitLab CI to provide agents with codebase context during automated review pipelines.

#### Architecture:
- A `neuron ci` command runs a one-shot, non-watching index build and outputs a `neuron-context.json` file.
- An AI agent (e.g., Claude Code in a GitHub Actions job) reads this file at the start of each run to get instant context without a live watcher.
- This enables **automated AI code review** as part of the CI/CD pipeline with full codebase awareness.

---
---

## Product B: Neuron Sessions™
**What it is:** A server-side personal memory daemon for LLMs. Solves cross-tab contamination and session amnesia across ALL AI interfaces.

> *"What Neuron Core does for code files, Neuron Sessions does for human minds."*

---

### The Problem (Stated Precisely)
Current LLMs (Gemini, GPT-4o, Claude) suffer from:
1. **Session Amnesia:** Every new conversation starts from absolute zero, even with the same user.
2. **Cross-Tab Contamination:** Two parallel tabs in Gemini do not know about each other and randomly bleed context together.
3. **Cold-Start Personalization:** Google Gems, Claude Memory, and OpenAI Memory require extensive prior usage before any personalization kicks in. They are reactive, not proactive.
4. **No Real-Time Context Streaming:** The LLM doesn't know what the user is doing *right now* in other applications.

---

### The Solution: Neuron Sessions Architecture

#### Core Concept
A **per-user session indexer** runs server-side (or locally). It maintains a structured SQLite ledger of the user's active sessions, behavioral signals, and working context. On every new LLM session start (new tab, new chat, new app launch), this ledger is queried and a **Context Injection Block** is prepended to the system prompt.

#### Three-Layer Architecture:

**Layer 1 — The Session Watcher (Real-time)**
- Watches active LLM tabs/conversations in real-time.
- Maintains a lightweight in-memory map of `{tab_id → current_topic_summary}`.
- When Tab A is asking about Python debugging, the watcher classifies it and labels it: `TAB_A: Python debugging, asyncio issue, user is frustrated`.
- When the user opens Tab B, the system prompt for Tab B immediately receives: `USER ACTIVE CONTEXT: Tab A is currently discussing Python asyncio debugging. Do not conflate responses from this session with that context.`

**Layer 2 — The Identity Ledger (Persistent)**
- A persistent SQLite database (`neuron_sessions.db`) per user.
- Stores:
  - **Behavioral Profile:** Communication style (formal/informal), expertise level per domain, preferred response length, language.
  - **Working Memory:** Active projects, goals the user has stated, decisions they have made.
  - **Episodic Memory:** Key moments from past sessions (e.g., "User solved a production outage on 2026-06-01 involving Redis timeout").
  - **Relationship Context:** Recurring topics, known collaborators, recurring problems.
- This database is updated continuously, not just at session end.

**Layer 3 — The Context Injection Protocol (The MCP Bridge)**
- Exposes a standard MCP-compatible interface: `get_user_context` tool.
- Any LLM (Gemini, GPT-4o, Claude) that supports MCP can call this tool at session start.
- Returns a structured, token-efficient context block (max 2,000 tokens to avoid wasting context budget):
  ```
  USER: Vishal Kudmethe | EXPERTISE: Rust/Systems, React, AI | STYLE: Direct, technical
  ACTIVE_TABS: [Tab A: Python debugging] [Tab B: THIS SESSION]
  RECENT_PROJECTS: Neuron Core (Rust MCP engine), AetherFlux (blockchain)
  CURRENT_GOALS: Launch Neuron Core™, achieve 5000 GitHub stars
  LAST_SESSION_CONTEXT: Discussed enterprise licensing strategy, domain ai-neuron.org live
  AVOID: Repeating explanations already given. User knows Rust, SQLite, MCP deeply.
  ```

---

### Why This is Architecturally Superior to Current Approaches

| Approach | Google Gems | OpenAI Memory | Neuron Sessions |
|---|---|---|---|
| **Personalization Speed** | Weeks of usage required | Days of usage required | Instant (from first login) |
| **Cross-Tab Awareness** | ❌ None | ❌ None | ✅ Real-time |
| **Privacy** | Cloud-stored, Google-owned | Cloud-stored, OpenAI-owned | User-owned SQLite, local or private server |
| **Model Agnostic** | Gemini only | GPT only | ✅ Any MCP-compatible LLM |
| **Update Latency** | Next session | Next session | ✅ Real-time, sub-second |
| **Audit Trail** | None | None | ✅ Full exportable log |

---

### Commercial Model for Neuron Sessions

**B2B (Sell to AI Labs directly):**
- License the Neuron Sessions protocol to Google, Anthropic, xAI, OpenAI as an infrastructure component.
- Pricing model: **Per Monthly Active User (MAU)** — e.g., \$0.001 per MAU per month.
- If Google deploys this for 100 million Gemini users → **\$100,000/month** licensing fee.

**B2C (Direct to Power Users):**
- Neuron Sessions daemon as a local app (\$9.99/month).
- Works across all their AI tools simultaneously — Gemini, ChatGPT, Claude, Copilot.
- A single user's "personal AI memory OS."

---

### Implementation Roadmap for Neuron Sessions

| Phase | Duration | Deliverable |
|---|---|---|
| **Sessions v0.1** | Week 1-2 | SQLite schema for Identity Ledger. CLI to write/read user context manually. |
| **Sessions v0.2** | Week 3-4 | MCP `get_user_context` tool. Plug into Claude Code for testing. |
| **Sessions v0.3** | Week 5-6 | Browser extension (Chrome/Firefox) that watches active LLM tabs and feeds context to the daemon. |
| **Sessions v1.0** | Week 7-8 | Full cross-tab awareness. Behavioral profiling engine. Commercial release. |

---

## The Combined Platform Vision: "Neuron Platform"

```
┌─────────────────────────────────────────────┐
│              NEURON PLATFORM™               │
│   "The Universal Memory Layer for AI"       │
├─────────────────┬───────────────────────────┤
│  Neuron Core™   │    Neuron Sessions™        │
│  (Code Memory)  │    (Human Memory)          │
│                 │                            │
│  For AI coding  │  For AI personal           │
│  agents that    │  assistants that           │
│  work with YOUR │  work with YOUR            │
│  codebase       │  life & context            │
├─────────────────┴───────────────────────────┤
│         Shared Infrastructure               │
│  • SQLite FTS5 Ledger Engine               │
│  • MCP JSON-RPC Protocol Layer             │
│  • Local-First Privacy Architecture        │
│  • AGPLv3 + Commercial Dual Licensing      │
└─────────────────────────────────────────────┘
```

**Acquisition Value of Combined Platform:**
- Neuron Core alone: \$30M – \$100M (developer tool)
- Neuron Sessions alone: \$200M – \$500M (AI infrastructure protocol)
- Combined Platform: **\$500M – \$1B+** (the universal memory OS for all AI)

---

## Immediate Next Actions

### This Week:
1. **Add `LICENSE-COMMERCIAL.md`** and enterprise contact to `neuron-core` repo.
2. **Add Enterprise section to `ai-neuron.org`** landing page.
3. **Begin `neuron_sessions.db` SQLite schema design** in a new `sessions/` module inside the Neuron workspace.
4. **File a provisional patent** (optional but recommended before any public disclosure of the Sessions architecture — consult an IP attorney).

### This Month:
1. Launch Neuron Core on Hacker News and Twitter.
2. Reach out to 5 enterprise engineering teams directly via LinkedIn for pilot conversations.
3. Ship Neuron Sessions v0.1 as a separate Rust binary (`neuron-sessions`).
4. Publish a technical blog post on `ai-neuron.org` describing the cross-tab coherence problem and Neuron Sessions as the solution — this will attract AI researcher and VC attention organically.
