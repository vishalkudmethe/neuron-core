#!/usr/bin/env bash
# Neuron v16 — Unix Global Installer
# Run from repository root: bash install.sh

set -e

BINARY_NAME="neuron"
SOURCE_BINARY="$(dirname "$0")/target/release/$BINARY_NAME"
INSTALL_DIR="$HOME/.local/bin"
INSTALL_PATH="$INSTALL_DIR/$BINARY_NAME"

# ── 1. Verify binary exists ────────────────────────────────────────────────────
if [ ! -f "$SOURCE_BINARY" ]; then
    echo "  [!] Release binary not found at $SOURCE_BINARY"
    echo "      Run: cargo build --release"
    exit 1
fi

# ── 2. Ensure install directory exists ────────────────────────────────────────
mkdir -p "$INSTALL_DIR"

# ── 3. Copy binary and make executable ───────────────────────────────────────
cp "$SOURCE_BINARY" "$INSTALL_PATH"
chmod +x "$INSTALL_PATH"
SIZE_KB=$(du -k "$INSTALL_PATH" | cut -f1)
echo "  [+] Installed: $INSTALL_PATH (${SIZE_KB} KB)"

# ── 4. Register in PATH ───────────────────────────────────────────────────────
SHELL_RC=""
if [ -f "$HOME/.zshrc" ]; then
    SHELL_RC="$HOME/.zshrc"
elif [ -f "$HOME/.bashrc" ]; then
    SHELL_RC="$HOME/.bashrc"
fi

if [ -n "$SHELL_RC" ]; then
    if ! grep -q "$INSTALL_DIR" "$SHELL_RC"; then
        echo "" >> "$SHELL_RC"
        echo "# Neuron — added by install.sh" >> "$SHELL_RC"
        echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$SHELL_RC"
        echo "  [+] Added $INSTALL_DIR to PATH in $SHELL_RC (restart terminal to activate)"
    else
        echo "  [i] $INSTALL_DIR already in PATH config"
    fi
fi

# ── 5. Sanity check ───────────────────────────────────────────────────────────
echo ""
echo "  Running sanity check..."
export PATH="$PATH:$INSTALL_DIR"
VERSION_OUT=$("$INSTALL_PATH" --version 2>&1)
echo "  [✓] $VERSION_OUT"
echo ""
echo "  ================================================"
echo "  Neuron v16 successfully installed and operational."
echo "  Run 'neuron --help' from any directory."
echo "  ================================================"
echo ""
