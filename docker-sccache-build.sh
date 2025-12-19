#!/bin/bash
# Build Thymos Docker image with host sccache support
#
# This script leverages BuildKit cache mounts to share the host's sccache
# with Docker builds, dramatically speeding up subsequent builds.

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
SCCACHE_DIR="${SCCACHE_DIR:-$HOME/.cache/sccache}"
IMAGE_NAME="${IMAGE_NAME:-thymos-agent}"
IMAGE_TAG="${IMAGE_TAG:-latest}"
DOCKERFILE="${DOCKERFILE:-Dockerfile.sccache}"

echo -e "${GREEN}=== Thymos Docker Build with sccache ===${NC}"
echo ""

# Check if Docker BuildKit is available
if ! docker buildx version >/dev/null 2>&1; then
    echo -e "${RED}Error: Docker BuildKit (buildx) is not available${NC}"
    echo "Please install Docker with BuildKit support or enable it:"
    echo "  export DOCKER_BUILDKIT=1"
    exit 1
fi

# Check if sccache is installed on host
if ! command -v sccache &> /dev/null; then
    echo -e "${YELLOW}Warning: sccache not found on host${NC}"
    echo "Install with: cargo install sccache"
    echo "The Docker build will still work but won't share cache with host."
    echo ""
fi

# Show current sccache stats (if available)
if command -v sccache &> /dev/null; then
    echo -e "${GREEN}Current host sccache statistics:${NC}"
    sccache --show-stats || true
    echo ""
fi

# Create sccache directory if it doesn't exist
mkdir -p "$SCCACHE_DIR"

echo -e "${GREEN}Building Docker image with sccache...${NC}"
echo "  Image: $IMAGE_NAME:$IMAGE_TAG"
echo "  Dockerfile: $DOCKERFILE"
echo "  sccache directory: $SCCACHE_DIR"
echo ""

# Build with BuildKit and cache mounts
# The --mount=type=cache in the Dockerfile will use BuildKit's cache
DOCKER_BUILDKIT=1 docker buildx build \
    --file "$DOCKERFILE" \
    --tag "$IMAGE_NAME:$IMAGE_TAG" \
    --progress=plain \
    --build-arg BUILDKIT_INLINE_CACHE=1 \
    .

echo ""
echo -e "${GREEN}=== Build Complete ===${NC}"

# Show updated sccache stats (if available)
if command -v sccache &> /dev/null; then
    echo ""
    echo -e "${GREEN}Updated host sccache statistics:${NC}"
    sccache --show-stats || true
fi

echo ""
echo -e "${GREEN}Docker image built successfully: $IMAGE_NAME:$IMAGE_TAG${NC}"
echo ""
echo "To run an agent:"
echo "  docker run -v thymos-data:/data $IMAGE_NAME:$IMAGE_TAG thymos agent create --id my_agent"
echo ""
echo "Or use docker-compose:"
echo "  docker-compose -f docker-compose.sccache.yml up"



