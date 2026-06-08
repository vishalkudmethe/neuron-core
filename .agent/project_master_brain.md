# Project Neuron — Master Brain
**Version**: v7 — Semantic Indexing & Agent Integration
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, logouts, account switches, directory switches, and machine migrations.

With v7, Neuron gains **true intelligence**: AST-based semantic symbol extraction, a live evolution ledger that tracks architectural tweaks per session, and an optimised `neuron context` output that compiles a maximum-information-density prompt block for external AI agents.

---

## 2. ARCHITECTURE OVERVIEW

```
┌────────────────────────────────────────────────────────────────┐
│                     NEURON v7 CORE ENGINE                      │
│                                                                │
│  ┌──────────┐  ┌───────────────────┐  ┌────────────────────┐  │
│  │  Watcher │  │  AST Parser       │  │  Project Manager   │  │
│  │ (notify) │  │ (tree-sitter +    │  │  (upward search +  │  │
│  │          │  │  custom Java/Dart)│  │   global index)    │  │
│  └────┬─────┘  └────────┬──────────┘  └────────┬───────────┘  │
│       │                 │                       │              │
│  ┌────▼─────────────────▼───────────────────────▼───────────┐  │
│  │               Unified Ledger (SQLite FTS5)               │  │
│  │   .neuron/index.sqlite  ←→  ~/.neuron/global_index.db    │  │
│  │   Columns: file_path, symbol_name, symbol_type,          │  │
│  │            semantic_intent, sha256                        │  │
│  └──────────────────────────────────────────────────────────┘  │
│  ┌──────────┐  ┌──────────────────┐  ┌────────────────────┐   │
│  │   Git    │  │ Evolution Ledger │  │   Loop Guardian    │   │
│  │(git2-rs) │  │ (manifest.json)  │  │  (loop_guard.rs)   │   │
│  └──────────┘  └──────────────────┘  └────────────────────┘   │
│                                                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │         Context Engine (session.rs)                     │   │
│  │  neuron context → v7 NEURON ACTIVE WORKSPACE CONTEXT    │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
```

---

## 3. v7 FEATURE SPECIFICATION

### 3.1 Incremental AST Indexer (`src/parser.rs`)

**Supported languages with full tree-sitter AST walkers:**
- **Rust** — `function_item`, `struct_item`, `enum_item`, `trait_item`; impl-block detection promotes functions to `Method`
- **Python** — `function_definition`, `class_definition`; class-scope detection for `Method` vs `Function`
- **JavaScript** — full tree-sitter walk, `function_declaration`, `class_declaration`, `method_definition`, `interface_declaration`
- **TypeScript/TSX** — same as JS using `tree_sitter_typescript::language_typescript()`

**Custom regex/line parsers (no tree-sitter crate available):**
- **Java** — class/interface/enum + method signature detection with Javadoc lookback
- **Dart** — class + method detection with doc-comment lookback

**Semantic intent extraction (`extract_preceding_comments`):**  
For every extracted symbol, the parser walks backward through source lines to collect `///`, `//`, `/** */`, and `/* */` comments immediately preceding the definition. This becomes the `semantic_intent` column in the FTS5 index.

**Hash-guarded indexing:**  
Before parsing, `process_file_change` queries the existing SHA-256 of the file. If unchanged → skip. This keeps `neuron watch` instant for unmodified files.

### 3.2 v7 Database Schema (`src/search.rs`)

```sql
CREATE TABLE memory_units (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL DEFAULT '',
    unit_type       TEXT NOT NULL,   -- 'file'|'function'|'method'|'struct'|'enum'|'trait'|'class'
    file_path       TEXT,            -- Absolute path
    symbol_name     TEXT,            -- Extracted symbol name
    symbol_type     TEXT,            -- Mirrors unit_type for FTS querying
    language        TEXT,
    content         TEXT,            -- Raw snippet (≤8KB)
    semantic_intent TEXT,            -- Extracted docstring/comment
    sha256          TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE VIRTUAL TABLE memory_fts USING fts5(
    id UNINDEXED, content, symbol_name, symbol_type, file_path, semantic_intent,
    content='memory_units', content_rowid='rowid'
);
```

**Auto-migration:** `bootstrap_local_db` detects old v5/v6 schema (presence of `path` column) and drops + recreates the tables transparently.

### 3.3 Evolution Ledger (`src/manifest.rs` + `src/watcher.rs`)

`NeuronManifest` now carries:
```json
{
  "top_level_intent": "A universal persistent memory layer...",
  "evolution_ledger": [
    {
      "timestamp": "2026-06-08 11:45 UTC",
      "file_path": "src/parser.rs",
      "tweak": "`src/parser.rs` — added 12 symbol(s)",
      "reason": "Detected by neuron watch file-change pipeline"
    }
  ]
}
```

The watcher appends one `EvolutionEntry` per meaningful file change. The ledger is capped at 50 entries (FIFO) to prevent unbounded growth.

### 3.4 `neuron context` v7 Output Format

```markdown
# NEURON ACTIVE WORKSPACE CONTEXT
> **Project:** [Name] | **Branch:** `[branch]` | **Memory Units:** [N]

## 🎯 TOP-LEVEL INTENT
[manifest.top_level_intent]

## 🛠️ RECENT ARCHITECTURAL TWEAKS (Last 3 Sessions)
*   **[timestamp]** - *Tweak:* [what changed] -> *Reason:* [why]

## 🧩 CRITICAL MODULES & SYMBOLS IN FOCUS
### Module: `[file_path]`
*   `[symbol_name]` ([symbol_type]) - *Intent:* [semantic_intent]
```

This block is also persisted to `.neuron/session_context.md`.

---

## 4. PROJECT DISCOVERY (v6 — carried forward)

**Tiered resolution strategy:**
1. **Upward traversal** — Walk from CWD toward filesystem root, looking for `.neuron/` directory
2. **Global index fallback** — Query `~/.neuron/global_index.sqlite` for most-recently-accessed project whose `.neuron/` directory still exists on disk

All commands (`restore`, `context`, `search`, `status`, `snapshot`, `backup`, `export`) use `discover_project_root()` automatically.

---

## 5. CLI REFERENCE

| Command | Description |
|---|---|
| `neuron init` | Initialize `.neuron/` in CWD, register in global index |
| `neuron watch` / `neuron start` | Start real-time watcher + incremental AST indexer |
| `neuron context` | Output v7 NEURON ACTIVE WORKSPACE CONTEXT block |
| `neuron restore` | Auto-discover project, print restored context |
| `neuron status` | Show project identity, memory unit count, loop guard state |
| `neuron switch <name>` | Switch to another globally-indexed project |
| `neuron list` | List all known projects from global index |
| `neuron search <query>` | FTS5 full-text search across symbols and content |
| `neuron snapshot` | Force-save current session to conversations/ |
| `neuron backup` | Manually trigger backup of `.neuron/` |
| `neuron export` | Export `.neuron/` as portable `.tar.gz` archive |

---

## 6. KEY FILES

| File | Role |
|---|---|
| `src/main.rs` | CLI entry point, command dispatch |
| `src/parser.rs` | Tree-sitter + custom AST symbol extractor with semantic intent |
| `src/search.rs` | SQLite FTS5 schema, schema migration, upsert, search |
| `src/watcher.rs` | File-system watcher, hash guard, evolution ledger writer |
| `src/session.rs` | Context compilation, `neuron context` v7 formatter |
| `src/manifest.rs` | NeuronManifest schema, EvolutionEntry, top_level_intent |
| `src/project_manager.rs` | `discover_project_root`, init, restore, switch, global index |
| `src/git.rs` | Branch, last commit, diff helpers |
| `src/loop_guard.rs` | Sliding-window loop detection and termination |
| `.neuron/index.sqlite` | Local FTS5 symbol database |
| `.neuron/neuron_manifest.json` | Project identity + evolution ledger |
| `.neuron/session_context.md` | Last-generated context block |
| `~/.neuron/global_index.sqlite` | Cross-project registry |

---

## 7. ROADMAP

| Version | Focus |
|---|---|
| **v6** ✅ | Production-ready core: upward path discovery, context restore, global index |
| **v7** ✅ | Semantic indexing: AST symbols, docstring extraction, evolution ledger, v7 context format |
| **v8** | Vector embeddings: populate `semantic_intent` BLOB with sentence-transformer embeddings for semantic search |
| **v9** | Team sync: `sync.rs` cloud-sync protocol for shared team memory |
| **v10** | Web dashboard: visual memory graph, timeline, symbol browser |

---

*This document is the canonical source of truth for Project Neuron architecture. Update before any major code change.*
