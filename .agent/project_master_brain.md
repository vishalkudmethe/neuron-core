# Project Neuron — Master Brain
**Version**: v14 — Self-Healing Daemon & Storage Optimizer
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, logouts, account switches, directory switches, and machine migrations.

With v14, Neuron reaches **Enterprise Hardening**. It implements an automated storage maintenance protocol (`neuron cleanup`), thread-safe crash-recovery lock handling for the file-watcher daemon, and a context-deduplication compiler (`src/dedup.rs`) that strips redundant structural definitions from multi-repo context payloads to maximize usable token budgets.

---

## 2. ARCHITECTURE OVERVIEW

```
┌────────────────────────────────────────────────────────────────────────┐
│                       NEURON v14 CORE ENGINE                           │
│                                                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Storage Maintenance (src/cleanup.rs)                           │   │
│  │  neuron cleanup                                                 │   │
│  │  VACUUM + ANALYZE on global + local SQLite databases            │   │
│  │  Log rotation: intent_log.json > 10MB → compress + 7-day trim  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Self-Healing Watcher Lock (src/watcher.rs)                     │   │
│  │  Writes PID to .neuron/watcher.lock on startup                  │   │
│  │  Validates PID on next start; evicts stale locks from dead PIDs │   │
│  │  WatcherLockGuard Drop impl removes lock file on clean exit     │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Deduplication Compiler (src/dedup.rs)                          │   │
│  │  Scans compiled stream prompt buffers for repeated definitions  │   │
│  │  Struct/class/enum body deduplication with pointer comment tags │   │
│  │  Runs automatically at end of compile_stream_context()          │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Busy-Timeout Lock Guards (all DB connections)                  │   │
│  │  .busy_timeout(1500ms) injected in search.rs, dependency.rs     │   │
│  │  and graph.rs — prevents concurrent process deadlocks           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  ┌──────────────────────────────────────────────────────────────┐      │
│  │  v11–v13 Core Engines (all preserved)                        │      │
│  │  dependency.rs / analyzer.rs / intent.rs / stream.rs         │      │
│  │  graph.rs / session.rs / watcher.rs / bridge.rs              │      │
│  └──────────────────────────────────────────────────────────────┘      │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. v14 FEATURE SPECIFICATION

### 3.1 Automated Storage & Log Maintenance (`src/cleanup.rs`)

Command: `neuron cleanup`

**Steps performed:**
1. Open global registry DB with `VACUUM;` + `ANALYZE;` to reclaim fragmented pages.
2. Open local project index DB with `VACUUM;` + `ANALYZE;`.
3. Check `.neuron/intent_log.json` size. If > 10MB: copy current log to `.neuron/intent_log.old.json`, then filter the live log to keep only entries with `last_modified` within 7 days.

### 3.2 Self-Healing Watcher Lock (`src/watcher.rs`)

**PID lock file lifecycle:**
- On `start_watcher()`: write PID to `.neuron/watcher.lock`.
- On next invocation: read PID, call `tasklist /FI` (Windows) or `kill -0` (Unix) to validate.
- If process is dead → evict stale lock silently and continue.
- If process is alive → bail with an informative error.
- `WatcherLockGuard` Drop impl removes the lock on any exit path (crash or clean).

### 3.3 Busy-Timeout Lock Guard (all DB connections)

All `SqliteConnectOptions` in `search.rs`, `dependency.rs`, and `graph.rs` now include:
```rust
.busy_timeout(std::time::Duration::from_millis(1500))
```
Prevents SQLite `SQLITE_BUSY` deadlocks when the watcher, bridge, and CLI run concurrently.

### 3.4 Context Deduplication Compiler (`src/dedup.rs`)

**Applied at:** End of `compile_stream_context()` in `stream.rs`.

**Algorithm:**
1. Regex-scan for `struct`/`class`/`enum`/`interface` declarations.
2. For each: extract the body between matched braces using a brace-counter.
3. Normalise body (strip whitespace) and build key `{Name}:{normalised_body}`.
4. First occurrence → emit verbatim.
5. Duplicate → replace body with pointer comment:
   ```rust
   // [Symbol body identical to <source_file>::<Name> - Deduplicated to save tokens]
   ```

---

## 4. CLI REFERENCE (v14 — Complete)

| Command | Flags | Description |
|---|---|---|
| `neuron cleanup` | | VACUUM databases, rotate intent logs, evict stale watcher locks |
| `neuron graph` | `--trace <symbol>` | Topology map or cascading mutation tracer |
| `neuron session` | `--track` | Start background intent tracker |
| `neuron log-error` | `--cmd` `--err` | Log execution error to telemetry pipeline |
| `neuron context` | `--export` `--include <alias>` | On-demand context with parent mutations |
| `neuron watch` / `start` | `--path --bridge` | Watcher + HTTP bridge |
| `neuron power-up <path>` | `--alias` | Ingest foreign workspace |
| `neuron link-deps` | `--parent --child --unlink --list` | Manage dependency arcs |
| `neuron analyze` | `--parent` | Structural mutation scan + impact matrix |
| `neuron switch` / `list` | | Switch or list registered workspaces |
| `neuron diagnose` | | Full environment & DB health audit |
| `neuron restore` | `--from` | Auto-discover + restore context |
| `neuron status` | | Status + PATH check |
| `neuron snapshot` | `--note` | Force snapshot |
| `neuron backup` | | Manual backup |
| `neuron export` | `--output` | Export `.tar.gz` archive |
| `neuron search <query>` | `--global --limit --interactive` | FTS5 search |

---

## 5. SECURITY & PERFORMANCE MODEL

| Concern | Mitigation |
|---|---|
| Concurrent DB access deadlocks | 1500ms busy-timeout on all SQLite connections |
| Stale watcher locks | PID validation + automatic eviction on startup |
| Intent log bloat (months of tracking) | 10MB threshold triggers 7-day rolling trim |
| Redundant struct/enum token waste in multi-repo context | AST-aware body deduplication with pointer tags |
| Source content in stream | Passed through `sanitize::sanitize_content()` |
| Bridge auth | Bearer token, same for `/v1/context` and `/v1/context/stream` |

---

*This document is the canonical source of truth for Project Neuron architecture. Update before any major code change.*
