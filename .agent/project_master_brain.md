# Project Neuron — Master Brain
**Version**: v6 — Production Ready Core
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, logouts, account switches, directory switches, and machine migrations.

With v6, Neuron focuses on production-readiness, robust upward-traversal project discovery, seamless restore/context generation for AI agents, and simplified global indexing to ensure a stable, zero-friction developer and AI-agent experience.

---

## 2. ARCHITECTURE OVERVIEW

```
┌────────────────────────────────────────────────────────┐
│                   NEURON CORE ENGINE                   │
│                                                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐ │
│  │ Watcher  │  │  Parser  │  │   Project Manager    │ │
│  │(notify)  │  │(tree-sitter)│ │  (multi-project)    │ │
│  └────┬─────┘  └────┬─────┘  └──────────┬───────────┘ │
│       │              │                   │              │
│  ┌────▼──────────────▼───────────────────▼───────────┐ │
│  │              Unified Ledger (SQLite)               │ │
│  │    index.sqlite  ←→  ~/.neuron/global_index.sqlite │ │
│  └────────────────────────────────────────────────────┘ │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐ │
│  │   Git    │  │  Search  │  │    Loop Guardian     │ │
│  │ (git2)   │  │ (sqlx FTS)│  │   (loop_guard.rs)   │ │
│  └──────────┘  └──────────┘  └──────────────────────┘ │
│  ┌──────────────────────────────────────────────────┐  │
│  │           Session Context (session.rs)           │  │
│  └──────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────┘
```

---

## 3. DIRECTORY STRUCTURE

```
.
├── .agent/
│   └── project_master_brain.md          ← You are here
├── .neuron/
│   ├── neuron_manifest.json             ← Project metadata + config
│   ├── conversations/                   ← Timestamped conversation snapshots
│   ├── index.sqlite                     ← Full local ledger (FTS + vectors)
│   ├── backups/                         ← Auto-backups before major changes
│   └── session_context.md              ← Human-readable restored context
├── Cargo.toml
└── src/
    ├── main.rs                          ← CLI entrypoint (clap)
    ├── watcher.rs                       ← Real-time file system + Git watcher
    ├── parser.rs                        ← tree-sitter multi-language parser
    ├── manifest.rs                      ← neuron_manifest.json R/W
    ├── conversation.rs                  ← Conversation snapshot engine
    ├── git.rs                           ← git2 integration
    ├── search.rs                        ← SQLite FTS5 full-text search
    ├── sync.rs                          ← Team sync stubs / export
    ├── loop_guard.rs                    ← Loop detection + termination
    ├── session.rs                       ← Session context + markdown generator
    ├── project_manager.rs               ← ★ v6: Project Discovery & Indexing
    └── utils.rs                         ← Shared helpers
```

---

## 4. PROJECT DISCOVERY LOGIC

Neuron implements a robust, tiered strategy for finding the correct project root:
1. **Upward Traversal**: Starting from the current working directory, walk recursively upward searching for a `.neuron/` directory. If found, the parent is the project root.
2. **Global Index Fallback**: If no `.neuron/` is found in the directory hierarchy, query `~/.neuron/global_index.sqlite` to find the most recently accessed project whose root path still exists on disk.
3. **Graceful Failure**: If no project can be discovered, output a helpful error message suggesting `neuron init` or `neuron switch <name>`.

All commands except `init` and `list` auto-run this discovery logic first to ensure they run against the correct project context.

---

## 5. CLI COMMAND REFERENCE

| Command | Alias / Alternative | Description |
|---------|---------------------|-------------|
| `neuron init` | — | Initialize `.neuron/` in current dir, register globally |
| `neuron status` | — | Show correct active project status, branch, and memory counts |
| `neuron list` | — | Show all known projects from global index |
| `neuron restore` | — | Upward search for nearest `.neuron/` (or global fallback) + reload context |
| `neuron context` | — | Generate `session_context.md` + print a clean Markdown block ready for copy-pasting to AI agents |
| `neuron watch` | `neuron start` | Run file watcher and loop guard in background |
| `neuron switch <target>` | — | Switch active project by name or path |
| `neuron search <query>` | — | Full-text search across current project memory |
| `neuron search -g <query>` | — | Full-text search across ALL known projects |
| `neuron snapshot` | — | Force-save current session to conversations/ |
| `neuron backup` | — | Manually trigger backup of .neuron/ |
| `neuron export` | — | Export .neuron/ as portable archive |

---

## 6. CORE MODULE RESPONSIBILITIES

### 6.1 `project_manager.rs` — Discovery & Switch Engine
- Manages global SQLite DB `~/.neuron/global_index.sqlite`.
- Stores absolute directory paths.
- Provides `discover_project_root()` to resolve active project paths.
- Handles project switches with path validity checks.

### 6.2 `session.rs` — Context & Agent Markdown Generator
- Writes human-readable `.neuron/session_context.md`.
- Compiles git state, recently updated files, and conversation snapshots.
- Formats ready-to-copy Markdown output for direct ingestion by AI models like Antigravity.

### 6.3 `watcher.rs` — File System Watcher Daemon
- Listens for file system notifications.
- Processes changes through Tree-sitter parsing and sqlite updates.
- Connects to Loop Guardian to halt on cyclic activity.

### 6.4 `loop_guard.rs` — Circularity & Loop Control
- Guards against repetitive agent actions by pausing watcher or warning users when thresholds are breached.

---

## 7. PATH ALIASES & PORTABILITY

When a `.neuron/` folder is transferred to a different machine or path:
1. Running `neuron restore` in the new path detects the folder.
2. Updates `neuron_manifest.json` and registers the new path alias in the global index.
3. Automatically fixes all paths relative to the new root for indexing consistency.

---

*This document is auto-updated by Neuron on each major implementation milestone.*
