#!/bin/bash
set -e

# Docker Build Script for Mayyam Backend
# Optimized for cross-platform builds with caching

# Default values
PLATFORM="linux/amd64"
TAG="mayyam-backend:latest"
PUSH=false
CACHE_DIR="./.buildx-cache"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --platform)
            PLATFORM="$2"
            shift 2
            ;;
        --tag)
            TAG="$2"
            shift 2
            ;;
        --push)
            PUSH=true
            shift
            ;;
        --multi-arch)
            PLATFORM="linux/amd64,linux/arm64"
            shift
            ;;
        --cache-dir)
            CACHE_DIR="$2"
            shift 2
            ;;
        *)
            echo "Usage: $0 [--platform PLATFORM] [--tag TAG] [--push] [--multi-arch] [--cache-dir DIR]"
            echo "Examples:"
            echo "  $0                                    # Build linux/amd64 locally"
            echo "  $0 --platform linux/arm64            # Build for ARM64"
            echo "  $0 --multi-arch --push               # Build multi-arch and push to registry"
            echo "  $0 --tag ghcr.io/user/app:v1.0.0     # Custom tag"
            exit 1
            ;;
    esac
done

echo "🚀 Building Mayyam Backend Docker Image"
echo "Platform: $PLATFORM"
echo "Tag: $TAG"
echo "Push: $PUSH"
echo "Cache dir: $CACHE_DIR"
echo ""

# Ensure buildx builder exists
if ! docker buildx inspect mayyam-builder &>/dev/null; then
    echo "📦 Creating buildx builder..."
    docker buildx create --use --name mayyam-builder
    docker buildx inspect --bootstrap
fi

# Build command
BUILD_ARGS=(
    "buildx" "build"
    "--platform" "$PLATFORM"
    "--tag" "$TAG"
)

# Add cache arguments
if [ -d "$CACHE_DIR" ]; then
    BUILD_ARGS+=("--cache-from" "type=local,src=$CACHE_DIR")
fi
BUILD_ARGS+=("--cache-to" "type=local,dest=$CACHE_DIR,mode=max")

# Add push or load
if [ "$PUSH" = true ]; then
    BUILD_ARGS+=("--push")
else
    BUILD_ARGS+=("--load")
fi

BUILD_ARGS+=(".")

echo "🔨 Running: docker ${BUILD_ARGS[*]}"
echo ""

# Start timing
start_time=$(date +%s)

# Execute build
docker "${BUILD_ARGS[@]}"

# Calculate build time
end_time=$(date +%s)
duration=$((end_time - start_time))
minutes=$((duration / 60))
seconds=$((duration % 60))

echo ""
echo "✅ Build completed successfully!"
echo "⏱️  Build time: ${minutes}m ${seconds}s"

# Show image size if built locally
if [ "$PUSH" = false ] && [[ "$PLATFORM" != *","* ]]; then
    echo "📊 Image size:"
    docker images "$TAG" --format "table {{.Repository}}:{{.Tag}}\t{{.Size}}"
fi
