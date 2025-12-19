#!/bin/bash
# Test script for Thymos WASM component with Locai server
#
# Prerequisites:
#   - Locai server running on http://localhost:3000
#   - Node.js installed
#   - npx available
#
# This script will:
#   1. Build the WASM component
#   2. Transpile to JavaScript using jco
#   3. Run the test

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WASM_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$WASM_DIR")"

echo "=== Thymos WASM Server Test ==="
echo ""

# Check for Node.js
if ! command -v node &> /dev/null; then
    echo "Error: Node.js not found. Please install Node.js."
    exit 1
fi

# Check for npx
if ! command -v npx &> /dev/null; then
    echo "Error: npx not found. Please install npm."
    exit 1
fi

# Check if Locai server is running
echo "Checking Locai server..."
if curl -s http://localhost:3000/api/health > /dev/null 2>&1; then
    echo "✓ Locai server is running on http://localhost:3000"
else
    echo "✗ Locai server not responding on http://localhost:3000"
    echo "  Please start Locai server first."
    exit 1
fi
echo ""

# Build WASM component
echo "Building WASM component..."
cd "$WASM_DIR"
cargo component build --release
echo "✓ Build complete"
echo ""

# Transpile to JavaScript
echo "Transpiling to JavaScript with jco..."
cd "$SCRIPT_DIR"

# Install jco if needed
if ! npx jco --version > /dev/null 2>&1; then
    echo "Installing @bytecodealliance/jco..."
    npm install @bytecodealliance/jco
fi

# Transpile with async instantiation
WASM_FILE="$PROJECT_ROOT/target/wasm32-wasip1/release/thymos_wasm.wasm"
if [ ! -f "$WASM_FILE" ]; then
    echo "Error: WASM file not found at $WASM_FILE"
    exit 1
fi

npx jco transpile "$WASM_FILE" -o ./thymos-js --instantiation async

echo "✓ Transpiled to ./thymos-js/"
echo ""

# Run the test
echo "Running test..."
echo ""
node test_server.mjs

