# Project Neuron Core — Public Release Notes (v1.0.0)

Official open-source release of Project Neuron Core, a local-first Model Context Protocol (MCP) host and code context engine designed to index and serve large workspaces to AI coding agents.

## Release Overview

| Metric | Detail |
|---|---|
| **Version** | v1.0.0 |
| **Release Date** | June 8, 2026 |
| **License** | GNU Affero General Public License v3 (AGPLv3) |
| **Crate Name** | `neuron-core` |
| **Official Repository** | [github.com/vishalkudmethe/neuron-core](https://github.com/vishalkudmethe/neuron-core) |
| **CI/CD Workflow** | Build & Release Binaries (GitHub Actions) |

---

## Core Capabilities

1. **Model Context Protocol (MCP) STDIO Bridge**: Runs a JSON-RPC 2.0 full-duplex protocol over standard I/O streams. Fully compatible with Claude Code, Cursor, and other agent platforms.
2. **AST-Level Symbol Indexing**: Parsers backed by Tree-sitter for Rust, Python, JavaScript, and TypeScript, storing structured symbols into a local SQLite index.
3. **Multi-Project Workspace Linker**: Traces parent-child repo topology and warns when upstream symbol definitions are changed.
4. **Local Sanitization Pipeline**: Scrubs database credentials, API keys, passwords, and SSH keys in memory before exporting data to reasoning models.
5. **Self-Healing Loop Guards**: Detects redundant terminal command cycles or file-write behaviors to prevent resource loop state.

---

## Technical Specifications

- **Footprint**: Compiled target size of ~10.46MB.
- **Resource Usage**: Under 15MB RAM idle, sub-millisecond local indexing.
- **Local SQLite Storage**: Path index database at `.neuron/index.sqlite` (ignored by version control).

---

## License & Compliance

Licensed under the **GNU Affero General Public License v3 (AGPLv3)**. Any modifications or hosting of this service on network servers requires publishing the source code under the same license terms.

---

## Download & Installation

Binaries are automatically built for the following platforms:
- **Windows (x86_64)**: `neuron-windows-x86_64.exe`
- **macOS (Apple Silicon)**: `neuron-macos-arm64`
- **macOS (Intel)**: `neuron-macos-intel-x86_64`

### Installation Instructions

**Windows:**
```powershell
# Place in bin directory within user profile
Move-Item neuron-windows-x86_64.exe C:\Users\$env:USERNAME\bin\neuron.exe
```

**macOS (Apple Silicon):**
```bash
chmod +x neuron-macos-arm64
sudo mv neuron-macos-arm64 /usr/local/bin/neuron
```
