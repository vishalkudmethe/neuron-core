# ─────────────────────────────────────────────────────────────────────────────
# AI-NEURON™  —  Multi-Stage Container Build
# Pillar 3: Cloud-Native Enterprise Deployment
#
#   Stage 1 (builder)  →  Full Rust toolchain, compiles ai-neuron binary
#   Stage 2 (runtime)  →  Minimal Debian slim, copies binary only (~20 MB image)
#
# Usage:
#   docker build -t ai-neuron:latest .
#   docker run -p 8080:8080 -v neuron-data:/root/.neuron ai-neuron:latest
# ─────────────────────────────────────────────────────────────────────────────

# ── Stage 1: Builder ─────────────────────────────────────────────────────────
FROM rust:1.78-slim-bookworm AS builder

# Install C build essentials and SQLite dev headers (required by sqlx)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy dependency manifests first for layer caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to pre-compile all dependencies
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true

# Now copy full source and rebuild (only changed files recompile)
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Runtime dependencies only (SQLite shared lib)
RUN apt-get update && apt-get install -y \
    libsqlite3-0 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -ms /bin/bash neuron

# Copy compiled binary from builder stage
COPY --from=builder /build/target/release/ai-neuron /usr/local/bin/ai-neuron

# Persistent data volume — stores .neuron/ index, audit logs, and license key
VOLUME ["/root/.neuron"]

# Default workspace mount point for enterprise codebases
VOLUME ["/workspace"]

WORKDIR /workspace

# Expose TCP MCP server port (enterprise/cloud mode)
EXPOSE 8080

# Health check — verifies the binary is responsive
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ai-neuron status || exit 1

# Default: start the TCP MCP server on port 8080
# Override with: docker run ai-neuron ai-neuron start-mcp  (for stdio mode)
ENTRYPOINT ["ai-neuron"]
CMD ["start-server", "--port", "8080"]
