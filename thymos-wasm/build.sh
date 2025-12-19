#!/bin/bash
# Build script for Thymos WASM Component
#
# Prerequisites:
#   cargo install cargo-component
#   rustup target add wasm32-wasip1
#
# This builds a WASM Component using WASI Preview 1 target.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "Building Thymos WASM Component..."
echo ""

# Check for required tools
if ! command -v cargo-component &> /dev/null; then
    echo "Error: cargo-component not found"
    echo "Install with: cargo install cargo-component"
    exit 1
fi

# Check for WASM target
if ! rustup target list --installed | grep -q "wasm32-wasip1"; then
    echo "Adding wasm32-wasip1 target..."
    rustup target add wasm32-wasip1
fi

# Build the component
echo "Building with cargo-component..."
cargo component build --release

echo ""
echo "Build complete!"
echo ""

# Check output
WASM_FILE="../target/wasm32-wasip1/release/thymos_wasm.wasm"
if [ -f "$WASM_FILE" ]; then
    SIZE=$(ls -lh "$WASM_FILE" | awk '{print $5}')
    echo "Output: target/wasm32-wasip1/release/thymos_wasm.wasm ($SIZE)"
    
    # Optionally show WIT interface if wasm-tools is available
    if command -v wasm-tools &> /dev/null; then
        echo ""
        echo "Exported interfaces:"
        wasm-tools component wit "$WASM_FILE" 2>/dev/null | grep -E "^  export" || true
    fi
else
    echo "Warning: Expected output not found"
    ls -la ../target/wasm32-wasip1/release/*.wasm 2>/dev/null || echo "No WASM files found"
fi

echo ""
echo "To use this component:"
echo "  - JavaScript: npx jco transpile thymos_wasm.wasm -o thymos-js"
echo "  - Wasmtime: wasmtime run --wasm component-model thymos_wasm.wasm"
echo "  - Go/Wazero: Use wazero.Runtime.CompileComponent()"

