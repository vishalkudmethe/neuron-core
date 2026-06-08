# Project Neuron — Master Brain
**Version**: v15 — Production Release Candidate
**Status**: PRODUCTION — STABLE
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It ships as a single, globally-installed binary (`neuron`) that runs on any workstation without runtime dependencies. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, directory migrations, and machine transfers.

**v15 marks the production GA release.** All v11–v14 engines are integrated, optimised, and locked. The binary is fully stripped, LTO-compiled, and globally deployed.

---

## 2. RELEASE BINARY PROPERTIES

```
Binary name   : neuron
Build profile : release (opt-level=3, lto=true, codegen-units=1, panic=abort, strip=true)
Install path  : %USERPROFILE%\bin\neuron.exe  (Windows)
               ~/.local/bin/neuron            (Unix)
Invocable from: any directory after PATH registration
```

---

## 3. COMPLETE CLI COMMAND MATRIX (v1 → v15)

| Command | Flags | Description |
|---|---|---|
| `neuron init` | | Initialise a new Neuron project in the current directory |
| `neuron restore` | `--from` | Auto-discover and restore context from global registry |
| `neuron start` | `--bridge` `--path` | Launch watcher + HTTP bridge in one shot |
| `neuron watch` | | Watch current project for live file changes |
| `neuron context` | `--export` `--include <alias>` | Generate on-demand AI context markdown (auto-injects parent mutations) |
| `neuron power-up <path>` | `--alias` | Ingest and index a foreign workspace into the global registry |
| `neuron switch <name>` | | Switch active project in the global registry |
| `neuron list` | `--long` | List all registered workspaces |
| `neuron search <query>` | `--global` `--limit` `--interactive` | Full-text FTS5 search across memory units |
| `neuron snapshot` | `--note` | Force-save a named session snapshot |
| `neuron status` | | Show project status, loop guard state, last session |
| `neuron backup` | | Manually backup `.neuron/` directory |
| `neuron export` | `--output` | Export `.neuron/` as portable `.tar.gz` archive |
| `neuron diagnose` | | Full environment and database health audit |
| `neuron link-deps` | `--parent --child --unlink --list` | Register or remove cross-project dependency arcs |
| `neuron analyze` | `--parent <alias>` | Scan parent for structural signature mutations + impact matrix |
| `neuron session` | `--track` | Start background live intent tracker (v12) |
| `neuron log-error` | `--cmd --err` | Pipe build/run stderr into telemetry for next stream payload (v12) |
| `neuron graph` | | Render ASCII topological dependency memory graph (v13) |
| `neuron graph` | `--trace <symbol>` | Trace cascading mutation from a symbol to all downstream call sites (v13) |
| `neuron cleanup` | | VACUUM databases, rotate intent logs, evict stale watcher locks (v14) |

---

## 4. HTTP BRIDGE ENDPOINTS

| Endpoint | Description |
|---|---|
| `GET /v1/context` | Full on-demand context payload (auth: Bearer token) |
| `GET /v1/context/stream` | Live focus-state stream with dedup pass and error injection (v12–v14) |

Bridge runs at `http://127.0.0.1:8089`. Token stored at `.neuron/bridge_token`.

---

## 5. GLOBAL INDEX SCHEMA (`.neuron/global_index.sqlite`)

| Table | Purpose |
|---|---|
| `projects` | Registry of all known workspaces (id, name, root_path, language, last_accessed) |
| `workspace_dependencies` | Directional parent→child dependency arcs |
| `signature_snapshots` | Symbol SHA-256 hashes for interface mutation tracking |

---

## 6. VERSION HISTORY SUMMARY

| Version | Milestone |
|---|---|
| v1–v4 | Core engine: watcher, parser, SQLite FTS5, session context |
| v5 | Multi-project global indexing and registry |
| v6–v9 | Conversation snapshots, git integration, bridge, backup/export |
| v10 | Universal folder ingestion (`power-up`), language detection |
| v11 | Cross-repo dependency topology + structural signature mutation engine |
| v12 | Live intent tracker, focus scoring, stream compiler, error telemetry |
| v13 | ASCII topology graph renderer + cascading mutation tracer |
| v14 | Self-healing PID watcher lock, busy-timeout guards, cleanup vacuum, AST dedup |
| **v15** | **Production release: LTO binary, global PATH deployment, install scripts** |

---

*This document is the canonical source of truth for Project Neuron. Neuron is production-complete.*
