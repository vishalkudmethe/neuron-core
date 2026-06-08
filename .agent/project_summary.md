# Project Neuron: Universal Persistent Memory Layer

Project Neuron is a high-performance, local-first utility designed to act as a **structural memory layer** and **context compiler** for AI coding agents (such as Claude Code, Cursor, Windsurf, and Copilot). It bridges the gap between raw codebase directories and the narrow, token-limited reasoning windows of modern LLMs.

---

## 1. The Problem
AI coding agents are highly capable but face structural limitations when working on complex, multi-project development environments:
*   **Context Window Saturation**: Codebases contain redundant boilerplate, documentation, and duplicate tokens. Agents quickly exhaust context limits or incur high token costs.
*   **Lack of Cross-Project Memory**: Agents struggle to coordinate updates across multiple repositories or distinct modules (e.g., updating a library and tracking its impact on downstream microservices).
*   **Security & Privacy Leaks**: Exposing raw configuration files, SSH keys, database credentials, or private proprietary algorithms to external LLM APIs risks severe security breaches.
*   **Autonomous Loop Hazards**: Agents can enter repetitive patterns (e.g., continuously modifying a file, running a failing compiler, and reverting) which wastes CPU cycles and API costs.
*   **Cloud Latency**: Existing context-sharing tools rely on external servers, adding latency, network overhead, and security vulnerabilities.

---

## 2. The Solution
Neuron resides directly on the developer's workstation as a self-contained, lightweight daemon and command-line engine. It parses, indexes, tracks, and filters project context locally:
*   **Local-First Indexing**: Uses Tree-sitter parsers to build an Abstract Syntax Tree (AST) map of classes, methods, and structures, storing them in a local SQLite database utilizing FTS5 (Full-Text Search).
*   **Official Model Context Protocol (MCP)**: Implements the MCP JSON-RPC 2.0 specification over standard I/O (`stdin`/`stdout`). This enables any modern IDE agent to discover and run Neuron tools programmatically without cloud servers.
*   **AST Structural Deduplication**: Compiles raw context data and filters it through a deduplication engine, leaving only the essential code topology and interfaces.
*   **Sanitization Filters**: Every piece of context passed to an agent is run through a sanitization pipeline that strips private tokens, credentials, and configuration secrets automatically.
*   **Dependency Cascading**: Tracks dependencies between parent libraries and consumer projects, warning agents when changing an upstream signature will break downstream implementations.

---

## 3. Salient Features

### 🔌 Official MCP Stdio Server Integration (`neuron start-mcp`)
Neuron serves as a plug-and-play Model Context Protocol host over local `stdio`. External agents auto-discover three high-powered tools:
*   `get_project_context`: Fetches a dense, fully deduplicated, and sanitized markdown context representing the workspace.
*   `search_symbols`: Runs ranked FTS5 queries across the database to locate files and symbol definitions.
*   `get_impact_graph`: Computes cascading downstream compilation impacts of a proposed symbol modification.

### 🌳 AST-Driven Code Indexing
Instead of reading files as flat text, Neuron parses source files to catalog specific code items (classes, structs, functions, enums). It indexes semantic declarations, signatures, and internal developer intents to build a clean index of structural units.

### ⛓️ Cross-Project Dependency Linker
Allows developers to build dependency trees between independent directories (e.g., `neuron link --parent <lib_path> --child <app_path>`). If an upstream signature changes, the impact graph tracks exactly which files in downstream workspaces are out-of-date or mutated.

### 🛡️ Private Data Sanitizer
An inline regex pipeline that filters out sensitive information:
*   API credentials, database connection strings, passwords, and private SSH/cryptographic keys are replaced with `<REDACTED_SECRET>` placeholders.

### 🔄 Intelligent Loop Guard
Monitors rapid, repetitive file system modifications or repeated compilation errors. If an agent triggers more than 5 identical loops within 60 seconds, Neuron flags the state to interrupt run cycles and prevent CPU/API waste.

### ⚡ Tailored Context Profiles
Supports profile parameters (`antigravity`, `claude`, `openai`) to shape context density:
*   **Antigravity Profile**: High density, providing deeper file counts and symbol ranges.
*   **Claude/OpenAI Profiles**: Compressed, shallow bounds optimized for cost-efficient token footprints.

### 🧹 Auto-Self-Healing & Maintenance
Includes native cleanup capabilities (`neuron cleanup`) which vacuums and optimizes SQLite databases, rotates system intent logs, and clears stale locks automatically to ensure the binary runs with zero overhead (~10MB footprint).
