# Neuron — Universal Persistent Memory Layer for AI Coding Agents

<div align="center">

```
  ███╗   ██╗███████╗██╗   ██╗██████╗  ██████╗ ███╗   ██╗
  ████╗  ██║██╔════╝██║   ██║██╔══██╗██╔═══██╗████╗  ██║
  ██╔██╗ ██║█████╗  ██║   ██║██████╔╝██║   ██║██╔██╗ ██║
  ██║╚██╗██║██╔══╝  ██║   ██║██╔══██╗██║   ██║██║╚██╗██║
  ██║ ╚████║███████╗╚██████╔╝██║  ██║╚██████╔╝██║ ╚████║
  ╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═══╝
```

**v5 — Multi-Project Persistent Memory**

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)](https://www.rust-lang.org/)
[![License: AGPLv3](https://img.shields.io/badge/License-AGPLv3-red.svg)](LICENSE)
[![License: Commercial](https://img.shields.io/badge/License-Commercial-brightgreen.svg)](LICENSE-COMMERCIAL.md)

</div>

---

## What is Neuron?

Neuron is a persistent, portable memory layer for AI coding agents. It remembers everything about your projects — code structure, conversations, decisions, git history — and restores full context instantly when you switch directories, restart your machine, or open a different project.

**Neuron survives:**
- Folder changes / directory switches
- PC restarts and logouts
- Account switches
- Machine migrations (copy `.neuron/` and it just works)

**Neuron v5 adds:**
- 🗂 **Multi-project support** — track unlimited projects simultaneously
- ⚡ **Instant switching** — `neuron switch myproject` restores full context in <1s
- 🌍 **Global index** — `~/.neuron/global_index.sqlite` knows all your projects
- 🔍 **Cross-project search** — find any symbol, decision, or conversation across all projects

---

## Quick Start

```bash
# 1. Build
cargo build --release

# 2. Initialize a new project
cd my-project
neuron init

# 3. Start the watcher (real-time indexing)
neuron watch

# 4. After any directory change, restore context instantly
neuron restore

# 5. See all your projects
neuron list

# 6. Switch to another project
neuron switch my-other-project
```

---

## CLI Reference

| Command | Description |
|---------|-------------|
| `neuron init` | Initialize `.neuron/` in current dir + register globally |
| `neuron watch` | Start real-time file + git watcher daemon |
| `neuron restore` | Auto-detect nearest `.neuron/` (upward search) + load context |
| `neuron switch <name\|path>` | Switch to another project by name or path |
| `neuron list` | Show all known projects |
| `neuron search <query>` | Full-text search across current project memory |
| `neuron search -g <query>` | Full-text search across ALL known projects |
| `neuron snapshot` | Force-save session snapshot to `conversations/` |
| `neuron status` | Show project state, memory count, git branch |
| `neuron backup` | Manually backup `.neuron/` |
| `neuron export` | Export `.neuron/` as portable archive |

---

## Architecture

```
.neuron/
├── neuron_manifest.json     # Project identity + config
├── index.sqlite             # Local ledger (FTS5 full-text search)
├── session_context.md       # Human-readable restored context
├── conversations/           # Timestamped session snapshots
└── backups/                 # Auto-backups before major ops

~/.neuron/
└── global_index.sqlite      # Global project registry (all machines)
```

### Core Modules

| Module | Purpose |
|--------|---------|
| `main.rs` | CLI entrypoint (clap) |
| `project_manager.rs` | ★ Multi-project: init, switch, list, restore |
| `watcher.rs` | Real-time FS + Git events (notify, debounced) |
| `parser.rs` | Symbol extraction (tree-sitter: Rust, Python, TS, JS) |
| `search.rs` | SQLite FTS5 ledger — upsert + ranked search |
| `session.rs` | Context restore — writes `session_context.md` |
| `loop_guard.rs` | Sliding-window loop detection + alert |
| `git.rs` | git2 integration — branch, commit indexing |
| `conversation.rs` | Timestamped markdown snapshots |
| `manifest.rs` | `neuron_manifest.json` R/W (atomic writes) |
| `sync.rs` | Export + team sync stubs (v7) |
| `utils.rs` | Hashing, paths, machine ID, backup, formatting |

---

## Memory Schema

### Local Index (`.neuron/index.sqlite`)

```sql
-- Every symbol, file, conversation, and git commit is a memory unit
CREATE TABLE memory_units (
    id          TEXT PRIMARY KEY,   -- UUID v4
    unit_type   TEXT NOT NULL,      -- 'file' | 'function' | 'struct' | 'git_commit' | ...
    path        TEXT,               -- File path
    symbol_name TEXT,               -- Function/struct name
    language    TEXT,               -- 'rust' | 'python' | 'typescript' | ...
    content     TEXT,               -- Content/snippet (capped 8KB)
    sha256      TEXT,               -- Content hash (dedup)
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- FTS5 virtual table for full-text search
CREATE VIRTUAL TABLE memory_fts USING fts5(
    id UNINDEXED, content, symbol_name, path,
    content='memory_units', content_rowid='rowid'
);
```

### Global Index (`~/.neuron/global_index.sqlite`)

```sql
-- All known projects across all machines
CREATE TABLE projects (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    root_path     TEXT NOT NULL UNIQUE,
    language      TEXT NOT NULL,
    last_accessed TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    tags          TEXT            -- JSON array
);

-- Portable path aliases per machine
CREATE TABLE path_aliases (
    project_id TEXT NOT NULL,
    machine_id TEXT NOT NULL,
    local_path TEXT NOT NULL,
    PRIMARY KEY (project_id, machine_id)
);
```

---

## Loop Guardian

Neuron actively prevents agent context loops. The Loop Guardian monitors a sliding window of file events and alerts when the same operation repeats suspiciously.

| Pattern | Threshold | Action |
|---------|-----------|--------|
| Identical file write | 5× in 60s | WARN + 5s pause |
| Full re-scan (unchanged) | 3× in 300s | HALT + alert |

---

## Portability

`.neuron/` is fully portable. To move a project to another machine:

```bash
# On machine A
neuron export          # or just copy .neuron/ manually

# On machine B (in the new project root)
neuron restore         # detects .neuron/, registers path alias, restores context
```

---

## Roadmap

| Version | Features | Status |
|---------|----------|--------|
| v5 | Multi-project manager, portable memory | ✅ **Current** |
| v6 | Vector embeddings + semantic search | 📋 Planned |
| v7 | Team sync + shared memory stores | 📋 Planned |
| v8 | Web dashboard + visual memory graph | 📋 Planned |

---

## Dependencies

- **tokio** — async runtime
- **sqlx** — SQLite + FTS5 (local ledger + global index)
- **notify** — cross-platform file system watching
- **tree-sitter** — multi-language symbol parsing
- **git2** — git integration
- **clap** — CLI
- **colored** / **tabled** — terminal UI

---

## Licensing

AI-NEURON is dual-licensed under the **GNU Affero General Public License (AGPLv3)** and a **Commercial Enterprise License**.

- **Open Source:** For personal use, educational use, and open-source projects, AI-NEURON is free under the terms of the [AGPLv3](LICENSE).
- **Commercial Use:** If you are embedding AI-NEURON inside closed-source proprietary software, shipping it within enterprise environments, or integrating it in OEM products, you must purchase a commercial license. See [LICENSE-COMMERCIAL.md](LICENSE-COMMERCIAL.md) for details.

For commercial inquiries, custom integrations, or SLAs, contact us at: **enterprise@ai-neuron.org**

---

*Built with Rust. Zero dependencies on external servers — all memory is local.*
