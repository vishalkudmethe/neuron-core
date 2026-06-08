# Project Neuron — Master Brain
**Version**: v13 — Topological Memory Graph & Visual State Engine
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, logouts, account switches, directory switches, and machine migrations.

With v13, Neuron implements **Zero-Dependency Terminal Visualization**. It translates complex multi-repo topologies, cross-project dependencies, real-time edit focal points, and interface signature mutations into a beautiful, ASCII-based terminal memory network graph (`neuron graph`) and interactive cascading mutation tracer (`neuron graph --trace <symbol>`).

---

## 2. ARCHITECTURE OVERVIEW

```
┌────────────────────────────────────────────────────────────────────────┐
│                       NEURON v13 CORE ENGINE                           │
│                                                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Topological Graph Engine (src/graph.rs)                        │   │
│  │  neuron graph                                                   │   │
│  │  Renders parent/child dependency DAG with box-drawing characters│   │
│  │  Integrates v12 intent logs to display activity hotspots       │   │
│  └──────────────────────────────┬──────────────────────────────────┘   │
│                                 │                                      │
│  ┌──────────────────────────────▼──────────────────────────────────┐   │
│  │  Cascading Mutation Tracer (src/graph.rs)                       │   │
│  │  neuron graph --trace <symbol>                                  │   │
│  │  Maps symbol declaration → lists all child calling sites        │   │
│  │  Appends [⚠️ MUTATED] warning for un-synced consumer files      │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  CLI Interface Router (src/main.rs)                              │   │
│  │  Dispatches `graph` command and `--trace` parameters            │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                        │
│  ┌──────────────────────────────────────────────────────────────┐      │
│  │  v11/v12 Core Engines                                        │      │
│  │  dependency.rs / analyzer.rs / intent.rs / stream.rs         │      │
│  └──────────────────────────────────────────────────────────────┘      │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. v13 FEATURE SPECIFICATION

### 3.1 Terminal Topological Graph Engine (`src/graph.rs`)

Command: `neuron graph`

**ASCII Rendering Canvas:**
- Gathers all projects from global index.
- Resolves DAG layers based on directional dependency arcs from `workspace_dependencies`.
- Formats each node box with name, language, and status indicators:
  - **High-Activity Hot-Spot (Cyan/Bold)**: If modified within 2 minutes (derived from `.neuron/intent_log.json`).
  - **Stale/Inactive (Dim/Low-Contrast)**: Untouched for more than 48 hours.
- Draws vertical and horizontal connector lines (`┌`, `─`, `┼`, `┐`, `▼`, `▲`) representing workspace relationships.

### 3.2 Interactive Mutation Tracer (`src/graph.rs`)

Command: `neuron graph --trace <symbol_name>`

**Cascading mutation analysis flow:**
1. Scan `signature_snapshots` in global index to find where `<symbol_name>` is declared.
2. Print symbol details: type, parent project, and its current signature hash.
3. Traverse downstream dependencies in the topology tree.
4. Open the local SQLite index of each child project, running FTS5 queries to identify all calling files containing references to the symbol.
5. Compare file modification times against the mutation's `changed_at` timestamp.
6. Print the tracing tree:
   - Mark files updated after the mutation as `[✓ Synced]`.
   - Mark files older than the parent signature mutation as `[⚠️ MUTATED] (At Risk)`.

---

## 4. CLI REFERENCE (v13)

| Command | Flags | Description |
|---|---|---|
| `neuron graph` | `--trace <symbol>` | Render interactive memory topology map or trace mutation cascade |
| `neuron session` | `--track` | Start background intent tracker (focus score poller) |
| `neuron log-error` | `--cmd` `--err` | Telemetry build-error logging |
| `neuron context` | `--export` `--include <alias>` | On-demand context with auto parent mutations |
| `neuron watch` / `start` | `--path --bridge` | Watcher + HTTP bridge |
| `neuron power-up <path>` | `--alias` | Ingest foreign workspace |
| `neuron link-deps` | `--parent --child --unlink --list` | Manage dependency arcs |
| `neuron analyze` | `--parent` | Mutation scan + impact matrix |
| `neuron switch` / `list` | | Switch workspace or list all known registries |
| `neuron diagnose` | | Health audit |

---

## 5. SECURITY & PERFORMANCE MODEL

- **DAG rendering safety**: Handles cyclic dependencies gracefully without infinite recursion by tracking visited node IDs in the hierarchy traversal.
- **Trace performance**: Scans child files using index lookups instead of slow directory grep operations.
- **Escape sequence compliance**: Uses ANSI terminal codes compatible with cross-platform shells (Windows PowerShell/cmd, Linux bash, macOS zsh).

---

*This document is the canonical source of truth for Project Neuron architecture. Update before any major code change.*
