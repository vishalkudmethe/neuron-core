# Project Neuron — Master Brain
**Version**: v12 — Live Intent Engine & Continuous Context Stream
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, logouts, account switches, directory switches, and machine migrations.

With v12, Neuron transitions from an **on-demand prompt generator** into a **continuous context streaming platform**. It monitors real-time developer activity, assigns focus scores to files based on edit recency, and serves dynamically assembled, proximity-aware context payloads via the local HTTP bridge — eliminating the need to manually re-run `neuron context` during active development sessions.

---

## 2. ARCHITECTURE OVERVIEW

```
┌────────────────────────────────────────────────────────────────────────┐
│                       NEURON v12 CORE ENGINE                           │
│                                                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Intent Tracker (src/intent.rs)                                 │   │
│  │  neuron session --track                                         │   │
│  │  Polls .neuron/intent_log.json for file modification recency    │   │
│  │  Focus Scores: modified <2m=HIGH, <10m=MED, else=LOW           │   │
│  └──────────────────────────────┬──────────────────────────────────┘   │
│                                 │                                      │
│  ┌──────────────────────────────▼──────────────────────────────────┐   │
│  │  Stream Compiler (src/stream.rs)                                │   │
│  │  GET /v1/context/stream (HTTP bridge, bearer auth)              │   │
│  │  Proximity Chunk Assembly:                                      │   │
│  │    1. Highest-scoring files (full source, capped)               │   │
│  │    2. Adjacent module symbol definitions                        │   │
│  │    3. Active v11 signature mutations on those paths             │   │
│  │    4. Active execution failure section (if log-error used)      │   │
│  │  Token Sliding Scale: profile token_cap / 6 per chunk          │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Error Telemetry (src/main.rs → intent.rs)                      │   │
│  │  neuron log-error --cmd <cmd> --err <stderr>                    │   │
│  │  Writes to .neuron/last_error.json                              │   │
│  │  Next stream payload includes 🔴 Active Execution Failure block │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  ┌──────────────────────────────────────────────────────────────┐      │
│  │  v11 Subsystems (all preserved)                              │      │
│  │  dependency.rs / analyzer.rs / bridge.rs / sanitize.rs       │      │
│  └──────────────────────────────────────────────────────────────┘      │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. v12 FEATURE SPECIFICATION

### 3.1 Asynchronous Activity & Intent Tracker (`src/intent.rs`)

Command: `neuron session --track`

**Focus scoring model:**

| Condition | Score |
|---|---|
| File modified within last 2 minutes | HIGH (100) |
| File modified within last 10 minutes | MEDIUM (50) |
| File is a registered parent dependency symbol file | MEDIUM (40) |
| All other indexed files | LOW (10) |

**Persistence:** Focus state serialised to `.neuron/intent_log.json` (updated on every poll cycle). Structure:

```json
{
  "updated_at": "2026-06-08T12:00:00Z",
  "entries": [
    { "file_path": "src/main.rs", "score": 100, "last_modified": "..." },
    ...
  ],
  "last_error": null
}
```

**Polling:** 15-second interval. Non-blocking tokio task. Reads file mtimes from OS, updates scores, writes JSON.

### 3.2 Live Stream Context Compiler (`src/stream.rs`)

Endpoint: `GET /v1/context/stream` — added alongside the existing `/v1/context` route in `bridge.rs`.

**Proximity chunk assembly algorithm:**
1. Load `.neuron/intent_log.json` — sort by score descending.
2. Take top N files (N = `token_cap / 15000`, minimum 1, maximum 5).
3. For each file: read full source content from disk, cap at `token_cap / (6 * N)` chars.
4. Query the local FTS5 index for top 3 symbol definitions in adjacent modules (files in the same directory).
5. Check `signature_snapshots` for any mutations on those file paths within 48h; append inline.
6. If `.neuron/last_error.json` exists and is less than 10 minutes old, inject a `🔴 Active Execution Failure` block.
7. Wrap in `<!-- NEURON_STREAM_START --> … <!-- NEURON_STREAM_END -->` delimiters.

**Token sliding scale:**
- `antigravity` profile: up to 15,000 chars per focal file chunk
- `claude` profile: up to 8,000 chars per focal file chunk
- `openai` profile: up to 3,000 chars per focal file chunk

### 3.3 Shell Execution Error Inflow (`src/main.rs`)

Command: `neuron log-error --cmd <command> --err <stderr_output>`

**Writes** `.neuron/last_error.json`:
```json
{
  "command": "cargo build",
  "stderr":  "error[E0308]: mismatched types ...",
  "logged_at": "2026-06-08T12:00:00Z"
}
```

The next `/v1/context/stream` response automatically picks this up and appends:
```
## 🔴 Active Execution Failure
**Command:** cargo build
**Error:**
error[E0308]: mismatched types ...
```

---

## 4. CLI REFERENCE (v12)

| Command | Flags | Description |
|---|---|---|
| `neuron session` | `--track` | Start background intent tracker (focus score poller) |
| `neuron log-error` | `--cmd` `--err` | Pipe build/run errors into intent state for next stream |
| `neuron context` | `--export` `--include <alias>` | On-demand context; auto-injects parent mutations |
| `neuron watch` / `start` | `--path --bridge` | Watcher + HTTP bridge (`/v1/context` + `/v1/context/stream`) |
| `neuron power-up <path>` | `--alias` | Ingest foreign workspace |
| `neuron link-deps` | `--parent --child --unlink --list` | Manage dependency arcs |
| `neuron analyze` | `--parent` | Structural mutation scan + impact matrix |
| `neuron restore` | `--from` | Auto-discover + restore context |
| `neuron status` | | Status + PATH check |
| `neuron diagnose` | | Full environment & DB health audit |
| `neuron switch <name>` | | Switch active project |
| `neuron list` | `--long` | All known projects |
| `neuron search <query>` | `--global --limit --interactive` | FTS5 search or interactive shell |
| `neuron snapshot` | `--note` | Force snapshot |
| `neuron backup` | | Manual backup |
| `neuron export` | `--output` | Export `.tar.gz` archive |

---

## 5. SECURITY & PERFORMANCE MODEL

| Concern | Mitigation |
|---|---|
| Intent log file size | Capped at 500 entries; oldest evicted |
| Error log staleness | Only injected if `last_error.json` < 10 minutes old |
| Source content in stream | Passed through `sanitize::sanitize_content()` before serving |
| Bridge auth | Same Bearer token as `/v1/context` |
| Background poller overhead | Tokio task with 15s sleep — zero blocking |
| Token overflow | Hard-capped per profile before any output is written |

---

*This document is the canonical source of truth for Project Neuron architecture. Update before any major code change.*
