# Project Neuron — Master Brain
**Version**: v16 — Universal Agentic MCP Integration over Stdio
**Status**: Active Development
**Last Updated**: 2026-06-08

---

## 1. MISSION STATEMENT

Neuron is the Universal Persistent Memory Layer for AI Coding Agents. It ships as a single, globally-installed binary (`neuron`) that runs on any workstation without runtime dependencies. It maintains complete, portable project memory (code, conversations, decisions, architecture) that survives folder changes, PC restarts, directory migrations, and machine transfers.

With v16, Neuron implements native support for the **Model Context Protocol (MCP)** via standard I/O streams (`neuron start-mcp`). This enables external AI agents and IDE tools (e.g., Cursor, Claude Code, Windsurf) to query project contexts, FTS5 search indexes, and mutation graphs directly over JSON-RPC 2.0 without local HTTP port contention or network-level configurations.

---

## 2. v16 MCP SERVER ARCHITECTURE

```
┌────────────────────────────────────────────────────────────────────────┐
│                        NEURON v16 MCP SERVER                           │
│                                                                        │
│  Standard Input (stdin)  ──► [JSON-RPC 2.0 Reader] ──┐                 │
│                                                      │                 │
│                                                      ▼                 │
│                                            [Tool Dispatch Router]      │
│                                                      │                 │
│                                                      ▼                 │
│  Standard Output (stdout) ◄── [JSON-RPC Writer] ◄────┘                 │
│                                                                        │
│  Standard Error (stderr)  ◄── [Diagnostics / System logs / Tracing]    │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Stdio JSON-RPC 2.0 Engine
- Starts using the command `neuron start-mcp`.
- All normal tracing and info print statements are redirected to `stderr` to avoid corrupting `stdout` JSON frames.
- Protocol supports `initialize`, `tools/list`, and `tools/call` methods.

### 2.2 Supported MCP Tools

1. **`get_project_context`**:
   - Calls the active context compilation pipeline (similar to `neuron context --export`).
   - Applies v14 AST deduplication compiler and passes output through the v14 sanitization pipeline.
   - Returns a dense project context markdown map.
2. **`search_symbols`**:
   - Takes parameter `query` (String).
   - Searches the active project registry and global DB for code declarations, file references, and symbols using SQLite FTS5.
   - Returns matched references.
3. **`get_impact_graph`**:
   - Takes parameter `symbol` (String).
   - Traces the cascading downstream dependency paths and mutation impact analysis of the specified symbol.
   - Returns the formatted structural impact matrix.

---

## 3. CLI REFERENCE (v16 — Complete)

| Command | Flags | Description |
|---|---|---|
| `neuron start-mcp` | | Start native Model Context Protocol (MCP) server over stdin/stdout |
| `neuron cleanup` | | VACUUM databases, rotate intent logs, evict stale watcher locks |
| `neuron graph` | `--trace <symbol>` | Topology map or cascading mutation tracer |
| `neuron session` | `--track` | Start background live intent tracker |
| `neuron log-error` | `--cmd --err` | Pipe build/run stderr into telemetry for next stream payload |
| `neuron context` | `--export` `--include <alias>` | Generate on-demand AI context markdown (auto-injects parent mutations) |
| `neuron watch` / `start` | `--path --bridge` | Watcher + HTTP bridge |
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

---

## 4. MCP JSON-RPC PROTOCOL SCHEMAS

### 4.1 `initialize` Response Result
```json
{
  "protocolVersion": "2024-11-05",
  "capabilities": {
    "tools": {}
  },
  "serverInfo": {
    "name": "neuron-mcp",
    "version": "1.0.0"
  }
}
```

### 4.2 `tools/list` Response Result
```json
{
  "tools": [
    {
      "name": "get_project_context",
      "description": "Get highly dense, deduplicated markdown prompt context of the active project.",
      "inputSchema": {
        "type": "object",
        "properties": {}
      }
    },
    {
      "name": "search_symbols",
      "description": "Search across workspace databases for symbols/files matching a query.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "query": {
            "type": "string",
            "description": "The search term or query pattern"
          }
        },
        "required": ["query"]
      }
    },
    {
      "name": "get_impact_graph",
      "description": "Trace cascading downstream mutation impact for a structural symbol.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Name of the symbol/method/struct to trace"
          }
        },
        "required": ["symbol"]
      }
    }
  ]
}
```

---

*This document is the canonical source of truth for Project Neuron architecture.*
