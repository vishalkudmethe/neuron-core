# Project Neuron — Master Brain
**Version**: v11 — Cross-Project Intelligence & Interface Propagator
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, logouts, account switches, directory switches, and machine migrations.

With v11, Neuron evolves from a parallel indexer into an **active cross-project structural dependency engine**. It tracks directional dependency arcs between workspaces, computes structural signature hashes of public symbols, detects breaking-change mutations in parent repositories, and automatically injects parent interface mutation warnings into any child project's AI context block — no `--include` flag required.

---

## 2. ARCHITECTURE OVERVIEW

```
┌───────────────────────────────────────────────────────────────────────┐
│                       NEURON v11 CORE ENGINE                          │
│                                                                       │
│  ┌──────────┐  ┌───────────────────┐  ┌─────────────────────────┐    │
│  │  Watcher │  │  AST Parser +     │  │  Project Manager v10    │    │
│  │ (notify) │  │  Signature Hasher │  │  power_up / register    │    │
│  │          │  │  (analyzer.rs)    │  │  resolve_alias          │    │
│  └────┬─────┘  └────────┬──────────┘  └────────────┬────────────┘    │
│       │                 │                           │                 │
│  ┌────▼─────────────────▼───────────────────────────▼──────────────┐  │
│  │                  Unified Ledger (SQLite FTS5)                   │  │
│  │   .neuron/index.sqlite  ←→  ~/.neuron/global_index.sqlite       │  │
│  │   + workspace_dependencies arc table                            │  │
│  │   + signature_snapshots mutation log                            │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  Dependency Topology (dependency.rs)                            │  │
│  │  neuron link-deps --parent <alias> --child <alias>              │  │
│  │  workspace_dependencies: (parent_id → child_id) arcs           │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  Signature Mutation Engine (analyzer.rs)                        │  │
│  │  Computes SHA-256 of symbol signature on each index pass        │  │
│  │  Detects shape changes → marks child files as High-Impact       │  │
│  │  neuron analyze --parent <alias>                                │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  Cascading Intelligence Injection (session.rs)                  │  │
│  │  Auto-detects parent mutations in last 48h on `neuron context`  │  │
│  │  Injects ⚠️ Parent Interface Mutations block automatically      │  │
│  └─────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────────┘
```

---

## 3. v11 FEATURE SPECIFICATION

### 3.1 Multi-Repo Dependency Topology Linking (`src/dependency.rs`)

Command: `neuron link-deps --parent <alias> --child <alias>`

**Global Schema additions (`~/.neuron/global_index.sqlite`):**

```sql
CREATE TABLE workspace_dependencies (
    id          TEXT PRIMARY KEY,
    parent_id   TEXT NOT NULL,   -- project id of the upstream/library workspace
    child_id    TEXT NOT NULL,   -- project id of the consumer workspace
    created_at  TEXT NOT NULL,
    UNIQUE (parent_id, child_id)
);

CREATE TABLE signature_snapshots (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL,
    symbol_name     TEXT NOT NULL,
    symbol_type     TEXT NOT NULL,
    signature_hash  TEXT NOT NULL,   -- SHA-256 of canonical signature string
    last_seen_at    TEXT NOT NULL,
    changed_at      TEXT,            -- NULL if no change detected yet
    UNIQUE (project_id, symbol_name)
);
```

**Operations:**
- `link_deps(parent_alias, child_alias)` — validates both aliases exist in the registry, inserts the arc.
- `list_deps(alias)` — prints all parent and child arcs for a workspace.
- `unlink_deps(parent_alias, child_alias)` — removes the arc.
- `get_parent_ids(child_project_id)` — returns all parent project IDs for a given child (used by `session.rs`).

### 3.2 Cross-Project Signature Mutation Tracker (`src/analyzer.rs`)

Command: `neuron analyze --parent <alias>`

**Signature hashing rules:**
- For a `Function`/`Method`: hash = SHA-256 of `"fn {name}({param_types}) -> {return_type}"` — extracted from the symbol snippet via lightweight regex.
- For a `Struct`: hash = SHA-256 of the sorted field name list + types string.
- For an `Enum`: hash = SHA-256 of sorted variant name list.
- For other symbol kinds: hash = SHA-256 of the raw snippet, capped at 512 chars.

**Mutation detection flow:**
1. Open the parent workspace's `index.sqlite`.
2. For each non-file symbol, compute the current signature hash.
3. Compare against `signature_snapshots`. If the hash differs → mark `changed_at = now`.
4. For every changed symbol, query each child workspace's FTS5 index for any `memory_units` whose `content` contains the symbol name.
5. Print a structured **Impact Matrix** table:

```
╭─────────────────────────────────────────────────────────╮
│  IMPACT MATRIX — Parent: aether → Children analysed: 2  │
├────────────────────┬──────────┬────────────────────────┤
│  Symbol            │ Change   │  At-Risk Files          │
├────────────────────┼──────────┼────────────────────────┤
│  LedgerEntry       │ Struct ↻ │  wallet-ui/ledger.ts   │
│  process_transfer  │ Fn sig ↻ │  relay/handler.rs      │
╰────────────────────┴──────────┴────────────────────────╯
```

### 3.3 Cascading Intelligence Injection (`src/session.rs`)

**Auto-inject on `neuron context` (no flag required):**
1. At context-generation time, look up the current project's ID in the global index.
2. Call `dependency::get_parent_ids()` to find all registered parent workspaces.
3. For each parent, query `signature_snapshots WHERE changed_at IS NOT NULL AND changed_at > (now - 48h)`.
4. If any mutations found → build a `## ⚠️ Parent Interface Mutations` markdown block listing each changed symbol, its type, and the timestamp of change.
5. Append the block to the context output — always, even without `--include`.

---

## 4. CLI REFERENCE (v11)

| Command | Flags | Description |
|---|---|---|
| `neuron init` | `--name --language` | Init project + PATH check |
| `neuron watch` / `start` | `--path --bridge` | Watcher + optional HTTP bridge |
| `neuron context` | `--export` `--include <alias>` | Context block; auto-injects parent mutations |
| `neuron power-up <path>` | `--alias <name>` | Ingest any directory |
| `neuron link-deps` | `--parent <alias>` `--child <alias>` | Register dependency arc |
| `neuron analyze` | `--parent <alias>` | Scan for structural mutations + impact matrix |
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

## 5. SECURITY MODEL SUMMARY

| Layer | Mechanism |
|---|---|
| Pre-index sanitization | `sanitize::sanitize_content()` on all content, snippets, and intents |
| Cross-project pull | `sanitize_content()` applied on foreign symbol names and intents |
| Signature hashing | SHA-256 of canonical form — no raw code stored in snapshot table |
| Bridge auth | Bearer token per session, stored in `.neuron/bridge_token` |
| Credential patterns | PEM keys, `api_key=`, `password=`, `secret=`, AWS creds, DB URIs |
| .gitignore compliance | `ignore::WalkBuilder` used in all crawl operations |

---

*This document is the canonical source of truth for Project Neuron architecture. Update before any major code change.*
