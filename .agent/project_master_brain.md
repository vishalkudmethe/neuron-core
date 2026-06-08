# Project Neuron — Master Brain
**Version**: v9 — Integration Bridge & Secure Profile Sandbox
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, logouts, account switches, directory switches, and machine migrations.

With v9, Neuron adds a local HTTP loopback server (Integration Bridge) to eliminate clipboard dependencies, customizable token budget profiles (Neuron.toml), and an intellectual property guard that sanitizes private keys and credentials before saving metadata to SQLite.

---

## 2. ARCHITECTURE OVERVIEW

```
┌────────────────────────────────────────────────────────────────┐
│                     NEURON v9 CORE ENGINE                      │
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
│  │  Diagnostics     │  │  Privacy Guard & Data Stripper  │    │
│  │  (neuron diagnose│  │  (src/sanitize.rs regex mask)   │    │
│  │   utils.rs)      │  │                                 │    │
│  └──────────────────┘  └─────────────────────────────────┘    │
│                                                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  HTTP Integration Bridge (src/bridge.rs)                │   │
│  │  GET /v1/context (requires Bearer token auth)          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Token Profile Budgeting (src/config.rs)                │   │
│  │  Neuron.toml -> antigravity / claude / openai presets    │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
```

---

## 3. v9 FEATURE SPECIFICATION

### 3.1 Localized AI Integration Bridge (`src/bridge.rs`)

Starts with `neuron watch --bridge` or `neuron start --bridge`.

**Features:**
1. Launches a background loopback HTTP server on `127.0.0.1:8089`.
2. Generates a secure, single-use Bearer token on startup, saved to `.neuron/bridge_token`.
3. Serves `GET /v1/context`, returning the compiled prompt context wrapped in `<!-- NEURON_CONTEXT_START -->` / `<!-- NEURON_CONTEXT_END -->` delimiters.
4. Requires header `Authorization: Bearer <token>`.

### 3.2 Advanced Token Budgeting & Profiling (`src/config.rs`)

Loads configuration from `Neuron.toml` at the project root.

**Profiles Matrix:**
- **`profile = "antigravity"`** (Default): Maximize symbol granularity (up to 8 files x 6 symbols per file), and include evolution ledger history up to the 50-item cap.
- **`profile = "claude"`**: Balanced granularity (5 files x 3 symbols), ignores evolution ledger history.
- **`profile = "openai"`**: Condenses definitions (3 files x 2 symbols), includes only high-level class, enum, and struct declarations to fit minimal token windows.

### 3.3 Intellectual Property Guard & Data Stripping (`src/sanitize.rs`)

Protects sensitive repository data before database persistence.

**Rules:**
- Identifies and strips RSA, EC, and general PEM private key blocks.
- Identifies assignments to `api_key`, `secret`, `password`, `auth_token`, and similar strings.
- Replaces matches with `[PRIVATE_KEY_REDACTED]` or `[SECRET_REDACTED]`.
- Scrubs connection strings matching standard DBMS URIs.

---

## 4. CLI REFERENCE (v9)

| Command | Flags | Description |
|---|---|---|
| `neuron init` | `--name --language` | Init project + PATH check |
| `neuron watch` / `start` | `--path --bridge` | Watcher, AST indexer & loopback HTTP bridge |
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

*This document is the canonical source of truth for Project Neuron architecture. Update before any major code change.*
