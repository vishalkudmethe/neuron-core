# Project Neuron — Master Brain
**Version**: v10 — Universal Workspace Ingestion & Cross-Project Inflow
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, logouts, account switches, directory switches, and machine migrations.

With v10, Neuron becomes a **cross-project domination engine**: it can ingest any external workspace directory into its global registry (`neuron power-up`), switch fluidly between indexed projects, and compile unified, multi-repository AI prompt blocks on demand (`neuron context --include <alias>`).

---

## 2. ARCHITECTURE OVERVIEW

```
┌───────────────────────────────────────────────────────────────────────┐
│                       NEURON v10 CORE ENGINE                          │
│                                                                       │
│  ┌──────────┐  ┌───────────────────┐  ┌─────────────────────────┐    │
│  │  Watcher │  │  AST Parser       │  │  Project Manager v10    │    │
│  │ (notify) │  │ (tree-sitter +    │  │  power_up / resolve_alias│   │
│  │          │  │  custom Java/Dart)│  │  detect_primary_language │   │
│  └────┬─────┘  └────────┬──────────┘  └────────────┬────────────┘    │
│       │                 │                           │                 │
│  ┌────▼─────────────────▼───────────────────────────▼──────────────┐  │
│  │                  Unified Ledger (SQLite FTS5)                   │  │
│  │   .neuron/index.sqlite  ←→  ~/.neuron/global_index.sqlite       │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  ┌──────────────────────┐  ┌─────────────────────────────────────┐   │
│  │  Diagnostics         │  │  Privacy Guard & Data Stripping     │   │
│  │  (neuron diagnose)   │  │  (sanitize.rs — runs pre-upsert)    │   │
│  └──────────────────────┘  └─────────────────────────────────────┘   │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  HTTP Integration Bridge (bridge.rs)                            │  │
│  │  GET /v1/context → Bearer token auth → delimited markdown out   │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  Token Profile Budgeting (config.rs / Neuron.toml)             │  │
│  │  antigravity: 250k / claude: 100k / openai: 30k               │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  Cross-Project Context Synthesis (session.rs)                   │  │
│  │  neuron context --include <alias> → multi-DB secondary pools    │  │
│  └─────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────────┘
```

---

## 3. v10 FEATURE SPECIFICATION

### 3.1 Universal Power-Up Pipeline (`src/project_manager.rs`)

Command: `neuron power-up <path> --alias <name>`

**Execution flow:**
1. `canonicalize()` the target path to an absolute, portable anchor.
2. Scaffold `.neuron/conversations/` and `.neuron/backups/` if absent.
3. Run `detect_primary_language()` — counts file extensions via `ignore::WalkBuilder`, picks the most common parseable language.
4. Write a default `Neuron.toml` (`profile = "antigravity"`) if missing.
5. Call `search::bootstrap_local_db()` to create `index.sqlite`.
6. Walk all non-build, non-git source files → `parser::extract_symbols()` → `sanitize::sanitize_content()` → `search::upsert_file()`.
7. Create or load `neuron_manifest.json`, write `session_context.md`, and `register()` in `~/.neuron/global_index.sqlite`.

**Security rules:**
- All symbol snippets and semantic intents pass through `sanitize_content` before any SQL insertion.
- Skips `\target\`, `.git`, `.neuron`, and `.tmp` paths during the crawl.
- Crawl respects `.gitignore` rules via `ignore::WalkBuilder`.

### 3.2 Global Workspace Portability (`src/project_manager.rs`)

- **`resolve_alias(alias)`** — looks up a registered workspace by name from the global SQLite index, returns its absolute `PathBuf`. Used by `session.rs` for cross-project pool resolution.
- **`switch_project`** — updates `last_accessed` timestamp so the global registry always reflects the most recently active workspace.
- **`detect_primary_language`** — async file-extension frequency analyzer, no external dependency.

### 3.3 Cross-Project Context Synthesis (`src/session.rs`)

Command: `neuron context --include <alias1> --include <alias2> ...`

**Multi-database engine:**
1. For each `--include` alias, calls `resolve_alias()` to get the external workspace root.
2. Opens a read-only secondary `SqlitePool` to that workspace's `index.sqlite`.
3. Pulls top 3 files × 3 symbols (high-level), sanitizes name and intent strings.
4. Loads that workspace's `neuron_manifest.json` and appends the last 2 evolution ledger entries.
5. Appends a `## 🔗 Cross-Project: <alias>` section to the unified context block.
6. **Token cap guard:** if `cross_project_md.len() > config.token_cap / 4`, truncation is applied and the loop breaks.
7. Missing or un-indexed aliases produce a warning section rather than a hard failure.

### 3.4 Command Interface Changes (`src/main.rs`)

| New / Changed | Flag | Description |
|---|---|---|
| `Commands::PowerUp` | `<path>` `--alias` | Ingest foreign workspace into global registry |
| `Commands::Context` | `--include <ALIAS>` (repeatable) | Merge named workspace into context output |

---

## 4. CLI REFERENCE (v10)

| Command | Flags | Description |
|---|---|---|
| `neuron init` | `--name --language` | Init project + PATH check |
| `neuron watch` / `start` | `--path --bridge` | Watcher + optional loopback HTTP bridge |
| `neuron context` | `--export <path\|->` `--include <alias>` | Context block; merge external workspaces |
| `neuron power-up <path>` | `--alias <name>` | Ingest any directory into global registry |
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

## 5. PATHING & PORTABILITY RULES

- **Absolute anchoring**: `power_up` always calls `std::fs::canonicalize()` before any path is stored — no relative paths ever reach the database.
- **Cross-machine migration**: `path_aliases` table in the global index maps `(project_id, machine_id) → local_path`. Running `neuron init` or `neuron restore` in the new location re-registers the alias for the current machine.
- **Token budget guard**: the `/4` cross-project fraction ensures the primary workspace always has 75% of the token budget, preventing context flooding from included workspaces.

---

## 6. SECURITY MODEL SUMMARY

| Layer | Mechanism |
|---|---|
| Pre-index sanitization | `sanitize::sanitize_content()` on all content, snippets, and semantic intents |
| Cross-project pull | `sanitize_content()` applied again on foreign DB symbol names and intents |
| Bridge auth | Bearer token generated per session, stored in `.neuron/bridge_token` |
| Credential patterns covered | PEM private keys, `api_key=`, `password=`, `secret=`, AWS creds, DB connection URIs |
| .gitignore compliance | `ignore::WalkBuilder` used in `power_up` crawl and `detect_primary_language` |

---

*This document is the canonical source of truth for Project Neuron architecture. Update before any major code change.*
