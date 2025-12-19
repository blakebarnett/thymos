#!/bin/bash
# Helper script to run the Go example with proper library paths

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Check if library exists
if [[ "$OSTYPE" == "darwin"* ]]; then
    LIB_NAME="libthymos_go.dylib"
else
    LIB_NAME="libthymos_go.so"
fi

# Try release first, then debug
if [ -f "$WORKSPACE_ROOT/target/release/$LIB_NAME" ]; then
    LIB_DIR="$WORKSPACE_ROOT/target/release"
elif [ -f "$WORKSPACE_ROOT/target/debug/$LIB_NAME" ]; then
    LIB_DIR="$WORKSPACE_ROOT/target/debug"
else
    echo "Error: Library not found. Run ./build.sh first."
    exit 1
fi

# Set library path for runtime
export LD_LIBRARY_PATH="$LIB_DIR:$LD_LIBRARY_PATH"
export DYLD_LIBRARY_PATH="$LIB_DIR:$DYLD_LIBRARY_PATH"

echo "Using library from: $LIB_DIR"
echo ""

cd "$SCRIPT_DIR"
go run ./go/example
