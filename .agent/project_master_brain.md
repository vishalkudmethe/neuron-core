# Project Neuron — Master Brain
**Version**: v5 — Multi-Project Persistent Memory Layer
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives:

- Folder changes
- PC restarts / logouts
- Account switches
- Directory switches
- Machine migrations

Neuron actively prevents context loops and auto-restores full session context. With v5, it supports **multiple simultaneous projects** with fast switching and global indexing.

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
    ├── session.rs                       ← Session context restore
    ├── project_manager.rs               ← ★ v5: Multi-project global index
    └── utils.rs                         ← Shared helpers
```

---

## 4. MEMORY SCHEMA

### 4.1 Local Index (`.neuron/index.sqlite`)

```sql
-- Core memory units
CREATE TABLE memory_units (
    id          TEXT PRIMARY KEY,         -- UUID v4
    project_id  TEXT NOT NULL,            -- Foreign key to projects
    unit_type   TEXT NOT NULL,            -- 'file', 'function', 'conversation', 'decision', 'git_commit'
    path        TEXT,                     -- Relative file path
    symbol_name TEXT,                     -- Function/struct/trait name
    language    TEXT,                     -- 'rust', 'python', 'ts', etc.
    content     TEXT,                     -- Raw or summarized content
    sha256      TEXT,                     -- Content hash for dedup
    embedding   BLOB,                     -- Future: vector embedding
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Full-text search virtual table
CREATE VIRTUAL TABLE memory_fts USING fts5(
    id UNINDEXED,
    content,
    symbol_name,
    path,
    content='memory_units',
    content_rowid='rowid'
);

-- Cross-project references
CREATE TABLE cross_refs (
    id              TEXT PRIMARY KEY,
    source_project  TEXT NOT NULL,
    target_project  TEXT NOT NULL,
    source_unit     TEXT NOT NULL,
    target_unit     TEXT NOT NULL,
    ref_type        TEXT NOT NULL,        -- 'depends_on', 'copied_from', 'mentioned_in'
    created_at      TEXT NOT NULL
);

-- Loop guardian log
CREATE TABLE loop_events (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL,
    pattern     TEXT NOT NULL,
    count       INTEGER NOT NULL,
    first_seen  TEXT NOT NULL,
    last_seen   TEXT NOT NULL,
    terminated  INTEGER NOT NULL DEFAULT 0
);
```

### 4.2 Global Index (`~/.neuron/global_index.sqlite`)

```sql
-- All known projects across all machines
CREATE TABLE projects (
    id              TEXT PRIMARY KEY,     -- UUID v4
    name            TEXT NOT NULL,        -- Human-readable project name
    root_path       TEXT NOT NULL UNIQUE, -- Absolute path to project root
    neuron_path     TEXT NOT NULL,        -- Absolute path to .neuron/ folder
    language        TEXT,                 -- Primary language
    last_accessed   TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    tags            TEXT                  -- JSON array of tags
);

-- Machine portability: path aliases per machine
CREATE TABLE path_aliases (
    project_id      TEXT NOT NULL,
    machine_id      TEXT NOT NULL,
    local_path      TEXT NOT NULL,
    PRIMARY KEY (project_id, machine_id)
);
```

---

## 5. CLI COMMAND REFERENCE

| Command | Description |
|---------|-------------|
| `neuron init` | Initialize `.neuron/` in current dir, register globally |
| `neuron watch` | Start real-time file + git watcher daemon |
| `neuron restore` | Auto-detect nearest `.neuron/` (upward search) + load context |
| `neuron switch <name\|path>` | Switch active project by name or path |
| `neuron list` | Show all known projects from global index |
| `neuron search <query>` | Full-text search across current project memory |
| `neuron search -g <query>` | Full-text search across ALL known projects |
| `neuron snapshot` | Force-save current session to conversations/ |
| `neuron status` | Show current project, loop guard status, last session |
| `neuron backup` | Manually trigger backup of .neuron/ |
| `neuron export` | Export .neuron/ as portable archive |

---

## 6. CORE MODULE RESPONSIBILITIES

### 6.1 `watcher.rs` — Real-time File & Git Watcher
- Uses `notify` (cross-platform FS events) for debounced file change detection
- Watches `.git/HEAD`, `.git/COMMIT_EDITMSG`, refs for git events
- Sends events through `tokio::sync::mpsc` channel to core processing loop
- Respects `.gitignore` / `.neuronignore` via the `ignore` crate
- On change: hashes file, checks for delta, calls parser, updates SQLite

### 6.2 `parser.rs` — Multi-Language Symbol Extraction
- Uses `tree-sitter` with grammars for: Rust, Python, TypeScript, JavaScript, Go, C
- Extracts: function signatures, struct/class definitions, imports, docstrings
- Stores extracted symbols as `memory_units` with type `'function'` or `'type'`

### 6.3 `manifest.rs` — Project Manifest R/W
- Reads/writes `.neuron/neuron_manifest.json`
- Schema: `{ id, name, root_path, language, created_at, version, tags, config }`
- Called on `neuron init` to create, and on every `restore` to verify

### 6.4 `conversation.rs` — Conversation Snapshot Engine
- Saves session summaries to `.neuron/conversations/YYYY-MM-DD_HH-MM-SS.md`
- Records: decisions made, files changed, open questions, next steps
- Used by `session.rs` to reconstruct context on restart

### 6.5 `git.rs` — Git Integration
- Uses `git2` to enumerate commits, branches, diffs
- Stores commit metadata as `memory_units` with type `'git_commit'`
- Watches for branch switches — triggers context reload on switch

### 6.6 `search.rs` — Full-Text & Semantic Search
- SQLite FTS5 for full-text search across all memory units
- `search <query>` returns ranked results with file/line context
- Future: vector similarity search via embeddings column

### 6.7 `loop_guard.rs` — Loop Detection & Termination
- Maintains sliding window of recent operations (last 100 events)
- Detects: repeated identical file writes, circular imports, repeated conversation patterns
- On detection: logs to `loop_events`, emits `WARN`, halts if threshold exceeded (default: 5 repeats in 60s)

### 6.8 `session.rs` — Session Context Restore
- On startup: reads last session timestamp, loads most recent conversation snapshot
- Generates `session_context.md` with: project name, last files touched, open decisions, git branch, summary of last conversation
- Emits rich colored output to terminal on `neuron restore`

### 6.9 `project_manager.rs` — ★ v5 Multi-Project Manager
- Maintains `~/.neuron/global_index.sqlite` (created on first `neuron init`)
- `register(root_path)` → inserts or updates project in global index
- `switch(name_or_path)` → finds project, loads its manifest + session context
- `list()` → prints table of all known projects (name, path, last accessed)
- `restore()` → walks upward from CWD to find nearest `.neuron/`, loads it
- `detect_project_change(old_cwd, new_cwd)` → called when CWD changes; triggers restore if project changes

### 6.10 `utils.rs` — Shared Helpers
- `sha256_file(path)` → content hash
- `machine_id()` → deterministic ID from hostname + username
- `find_neuron_root(start_path)` → upward directory search for `.neuron/`
- `format_duration(d)` → human-readable duration
- `ensure_dir(path)` → mkdir -p equivalent

---

## 7. MULTI-PROJECT PORTABILITY

When a user copies `.neuron/` to another machine:
1. `neuron restore` detects the `.neuron/` folder
2. Reads `neuron_manifest.json` → extracts `root_path`
3. If `root_path` differs from current CWD, registers a path alias in `global_index.sqlite`
4. Prompts user: *"Detected project 'X' at new path '/new/path'. Registering as alias."*
5. All memory units remain valid (paths stored relative to root)

---

## 8. LOOP GUARDIAN — THRESHOLDS

| Pattern | Threshold | Action |
|---------|-----------|--------|
| Identical file write | 5× in 60s | WARN + pause watcher |
| Same conversation pattern | 3× | WARN user |
| Circular file dependency | 1× | LOG + skip |
| Full re-scan on unchanged dir | 3× in 300s | HALT + alert |

---

## 9. RELIABILITY PRINCIPLES

1. **Graceful Degradation**: If `.neuron/` is missing → suggest `neuron init` or `neuron restore`
2. **Auto-Backup**: Before any destructive operation, copy `.neuron/` to `.neuron/backups/`
3. **Atomic Writes**: All SQLite writes use transactions; manifest uses temp-file + rename
4. **Clear Errors**: All user-facing errors use `anyhow` with context chains, displayed in color

---

## 10. ROADMAP

| Phase | Milestone | Status |
|-------|-----------|--------|
| v1 | Core watcher + SQLite ledger | ✅ Designed |
| v2 | Parser + Git integration | ✅ Designed |
| v3 | Session context + restore | ✅ Designed |
| v4 | Loop guardian | ✅ Designed |
| v5 | Multi-project manager | 🔄 In Progress |
| v6 | Vector embeddings + semantic search | 📋 Planned |
| v7 | Team sync + export | 📋 Planned |
| v8 | Web dashboard | 📋 Planned |

---

*This document is auto-updated by Neuron on each major implementation milestone.*
