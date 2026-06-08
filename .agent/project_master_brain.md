# Project Neuron — Master Brain
**Version**: v8 — Runtime Execution & Agent Pipeline
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, logouts, account switches, directory switches, and machine migrations.

With v8, Neuron becomes **fully operational**: system PATH integration diagnostics, an interactive semantic query shell, a `neuron diagnose` safety auditor, and export-ready context blocks with agent-compatible delimiters.

---

## 2. ARCHITECTURE OVERVIEW

```
┌────────────────────────────────────────────────────────────────┐
│                     NEURON v8 CORE ENGINE                      │
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
│  └──────────────────────────────────────────────────────────┘  │
│                                                                │
│  ┌──────────────────┐  ┌─────────────────────────────────┐    │
│  │  Diagnostics     │  │  Interactive Query Shell        │    │
│  │  (neuron diagnose│  │  (neuron search --interactive)  │    │
│  │   utils.rs)      │  │   search.rs readline loop       │    │
│  └──────────────────┘  └─────────────────────────────────┘    │
│                                                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Context Engine (session.rs)                            │   │
│  │  neuron context --export [file]                         │   │
│  │  Includes <!--NEURON_CONTEXT_START/END--> delimiters    │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
```

---

## 3. v8 FEATURE SPECIFICATION

### 3.1 PATH Diagnostics (`src/utils.rs`)

`check_path_registration(binary_path: &Path)` runs after `neuron init` and `neuron status`.

**Logic:**
1. Resolve the current binary's path via `std::env::current_exe()`
2. Read the system `PATH` environment variable and split by `;` (Windows) or `:` (Unix)
3. Check if the binary's parent directory appears in any PATH entry
4. If **not found** → print a highlighted, OS-specific shell snippet:

```
⚠ neuron is not on your PATH.
To fix this permanently, run:

  PowerShell:  $env:PATH += ";D:\AI Neuron\target\release"
               [System.Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";D:\AI Neuron\target\release", "User")
  CMD:         setx PATH "%PATH%;D:\AI Neuron\target\release"
  Bash/Zsh:    export PATH="$PATH:/d/AI Neuron/target/release"
```

### 3.2 Interactive Search Shell (`src/search.rs`)

`search_interactive(project_root: &Path)` — activated by `neuron search --interactive`.

**Behaviour:**
- Opens the local FTS5 index
- Prints a prompt `neuron> ` and reads lines from stdin in a loop
- On each query: runs FTS5 MATCH, renders ranked results (symbol, type, file, semantic_intent)
- Special commands: `:q` / `:quit` → exit loop, `:help` → print help, empty line → re-prompt
- Uses `std::io::{stdin, stdout, Write}` — no external readline dependency

### 3.3 `neuron diagnose` (`src/utils.rs` + `src/main.rs`)

`run_diagnostics(project_root: Option<&Path>)` audits:

| Check | Green | Red |
|---|---|---|
| Binary on PATH | ✓ Found in PATH | ✗ Not on PATH + fix snippet |
| Global DB | ✓ Exists + readable | ✗ Missing or locked |
| Local DB | ✓ memory_units rows > 0 | ⚠ Empty — run `neuron watch` |
| Loop Guardian | ✓ No active loops | ⚠ N loop events in last window |
| Watcher processes | ✓ No zombie handles | ⚠ Cannot verify (no daemon PID) |

Output is a clean table using `tabled`.

### 3.4 `neuron context --export` (`src/session.rs`)

`print_agent_context(project_root, export_path: Option<&Path>)`:

- Wraps context block with agent-compatible HTML-style delimiters:
  ```
  <!-- NEURON_CONTEXT_START -->
  ...markdown...
  <!-- NEURON_CONTEXT_END -->
  ```
- If `--export <path>` provided: writes the delimited block to that file path, prints confirmation
- If `--export -`: writes to stdout only (pipe-friendly, no banner)
- Default (no flag): prints banner + block to terminal as before

---

## 4. PROJECT DISCOVERY (v6 — carried forward)

**Tiered resolution:** Upward traversal → Global index fallback.  
All commands use `discover_project_root()` automatically.

---

## 5. CLI REFERENCE (v8)

| Command | Flags | Description |
|---|---|---|
| `neuron init` | `--name --language` | Init project + PATH check |
| `neuron watch` / `start` | `--path` | Real-time watcher + AST indexer |
| `neuron context` | `--export <path\|->`  | v7 context block, optionally exported |
| `neuron restore` | `--from` | Auto-discover + restore context |
| `neuron status` | | Status + PATH check |
| `neuron diagnose` | | Full environment & DB health audit |
| `neuron switch <name>` | | Switch project |
| `neuron list` | `--long` | All known projects |
| `neuron search <query>` | `--global --limit --interactive` | FTS5 search or interactive shell |
| `neuron snapshot` | `--note` | Force snapshot |
| `neuron backup` | | Manual backup |
| `neuron export` | `--output` | Export `.tar.gz` archive |

---

## 6. KEY FILES (v8)

| File | Role |
|---|---|
| `src/main.rs` | CLI dispatch — now includes `Diagnose` command, `--interactive` search, `--export` context |
| `src/utils.rs` | PATH diagnostics, `run_diagnostics`, `check_path_registration` |
| `src/search.rs` | `search_interactive` readline loop |
| `src/session.rs` | `print_agent_context` with delimiter tags and `--export` support |
| `src/parser.rs` | AST symbol extractor (v7, unchanged) |
| `src/watcher.rs` | File watcher + evolution ledger (v7, unchanged) |
| `src/manifest.rs` | NeuronManifest + EvolutionEntry (v7, unchanged) |
| `src/project_manager.rs` | discover_project_root (v6, unchanged) |

---

## 7. ROADMAP

| Version | Focus |
|---|---|
| **v6** ✅ | Production-ready core: upward path discovery, context restore, global index |
| **v7** ✅ | Semantic indexing: AST symbols, docstring extraction, evolution ledger |
| **v8** ✅ | Runtime operationalization: PATH diagnostics, interactive search, diagnose, export |
| **v9** | Vector embeddings: sentence-transformer semantic search |
| **v10** | Team sync: cloud-sync protocol for shared team memory |
| **v11** | Web dashboard: visual memory graph, timeline, symbol browser |

---

*This document is the canonical source of truth for Project Neuron architecture. Update before any major code change.*
