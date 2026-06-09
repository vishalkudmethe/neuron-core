# AI-NEURON™: Phase 1 Launch & Hype Action Plan
**Target Milestone:** 5,000 GitHub Stars & 10,000 Weekly Active Users (WAU)

This action plan details the step-by-step roadmap for executing the launch of AI-NEURON™ starting tomorrow morning.

---

## Day 1: Asset Verification & Final Prep

### 1. Domain & SSL Verification
*   **Action:** Open `https://ai-neuron.org` in the browser.
*   **Verification:** Ensure DNS has fully propagated (no `NXDOMAIN` error) and that GitHub has successfully provisioned the SSL certificate (secure `https://` lock icon).

### 2. GitHub Release Verification (v1.0.0)
*   **Action:** Ensure the release workflow has run and successfully attached the three compiled binaries to the release tag:
    *   `neuron-windows-x86_64.exe`
    *   `neuron-macos-arm64`
    *   `neuron-macos-intel-x86_64`
*   **Action:** Click each download button on `https://ai-neuron.org` to verify it successfully downloads the correct binary from the public `neuron-core` repository.

### 3. Local Diagnostic Run
*   **Action:** Run `cargo install --path .` (or run the compiled binary) to ensure the version outputs `1.0.0`.
*   **Action:** Run `neuron watch` to verify the SQLite indexing runs flawlessly.

---

## Day 2: The Viral Launch (Hacker News & Twitter/X)

### 1. Hacker News (Show HN)
*   **Submission Title:** `Show HN: AI-NEURON – Local-first SQLite context engine for AI coding agents`
*   **Link:** `https://ai-neuron.org` (or directly to the GitHub repo `https://github.com/vishalkudmethe/ai-neuron`)
*   **Launch Time:** Target **8:00 AM EST (5:30 PM IST)** — this is the peak reading time for Hacker News.
*   **Founder's Intro Comment (Post immediately after submitting):**
    > *“Hey HN,*
    > *I built AI-NEURON because I was tired of AI agents (like Claude Code and Cursor) losing context of my codebase, running slow, or blowing through my API limits. AI-NEURON runs locally in the background, watches files, and builds a SQLite index of symbols and blast-radius mappings at native speed.*
    > *It exposes 5 standard MCP tools over stdio, keeps 100% of your data private, and automatically sanitizes credentials before sending them to the LLM. Written in Rust, open source under AGPLv3. Excited to hear what you think!”*

### 2. Twitter/X Thread Release
*   **Action:** Publish the 6-post thread mapped out in `.agent/acquisition_pitch_deck.md`.
*   **Engagement:** Tag developers, Rust engineers, and AI builders who frequently tweet about MCP, local-first software, and AI agents.

---

## Day 3-5: Targeted Community Infiltration

### 1. Reddit Promotion
Create highly technical, text-based posts detailing *how* it works (e.g., FTS5 index, SQLite performance) on the following subreddits:
*   **`/r/rust`** (Focus on the Rust implementation, low footprint)
*   **`/r/LocalLLaMA`** (Focus on local-first privacy, zero data exfiltration)
*   **`/r/selfhosted`** / **`/r/commandline`** (Focus on CLI utility)

### 2. AI Tool Communities
*   **Cursor Forum:** Post in the Cursor community showcase explaining how to connect AI-NEURON as an MCP tool inside Cursor.
*   **Claude Developer Forums:** Showcase how Claude Code can query AI-NEURON via stdio.

### 3. Tech Newsletters Outbound
Email or submit to the following developer newsletters (they are always looking for trending open-source dev-tools):
*   **Console.dev** (Focus on developer utilities)
*   **TLDR Web Dev**
*   **This Week in Rust**

---

## Day 6+: Community Feedback Loop & Star Retention

*   **Fast Response:** Monitor GitHub Issues and pull requests daily. Merging community PRs quickly builds massive goodwill and turns users into active project advocates.
*   **Star Counter:** Track the daily growth velocity of `https://github.com/vishalkudmethe/ai-neuron`.
