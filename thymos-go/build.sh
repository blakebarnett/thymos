#!/bin/bash
# Build script for Go bindings
# This builds the Rust library and generates C headers for Go CGO linking

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Building Thymos Go bindings..."
echo "Workspace: $WORKSPACE_ROOT"
echo ""

# Build the Rust library
echo "Step 1: Building Rust library..."
cd "$WORKSPACE_ROOT"
cargo build --release --package thymos-go

echo ""
echo "Step 2: Verifying build outputs..."

# Check for library
if [[ "$OSTYPE" == "darwin"* ]]; then
    LIB_NAME="libthymos_go.dylib"
else
    LIB_NAME="libthymos_go.so"
fi

if [ -f "$WORKSPACE_ROOT/target/release/$LIB_NAME" ]; then
    echo "✓ Library built: target/release/$LIB_NAME"
else
    echo "✗ Library not found: target/release/$LIB_NAME"
    exit 1
fi

# Check for generated header
if [ -f "$SCRIPT_DIR/include/thymos.h" ]; then
    echo "✓ Header generated: include/thymos.h"
else
    echo "⚠ Header not generated (may be normal if cbindgen skipped)"
fi

echo ""
echo "Build complete!"
echo ""
echo "Library location: $WORKSPACE_ROOT/target/release/$LIB_NAME"
echo ""
echo "To run the Go example:"
echo "  cd $SCRIPT_DIR"
echo "  ./run_example.sh"
echo ""
echo "To use in your Go project:"
echo "  export LD_LIBRARY_PATH=\"$WORKSPACE_ROOT/target/release:\$LD_LIBRARY_PATH\""
echo "  go run your_program.go"
