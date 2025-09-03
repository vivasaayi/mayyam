#!/bin/bash
# Quick Build Test Script - Tests different Docker build approaches

set -e

echo "🚀 Docker Build Solution Testing"
echo "================================"

# Test 1: Simple regular build (current architecture)
echo "📦 Test 1: Regular Docker build (host architecture)"
echo "Command: docker build -f Dockerfile.simple -t mayyam-backend:simple ."
echo "Expected: Works on Mac (arm64) and Linux (amd64/arm64)"
echo ""

# Test 2: Cross-platform build for linux/amd64
echo "📦 Test 2: Cross-platform build for Linux/AMD64"
echo "Command: docker buildx build --platform linux/amd64 -f Dockerfile.simple -t mayyam-backend:linux-amd64 --load ."
echo "Expected: Uses Confluent packages (fast), runs on Linux AMD64 servers"
echo ""

# Test 3: Multi-architecture build
echo "📦 Test 3: Multi-architecture build"
echo "Command: docker buildx build --platform linux/amd64,linux/arm64 -f Dockerfile.simple -t mayyam-backend:multi --push"
echo "Expected: Creates images for both architectures, pushes to registry"
echo ""

echo "🔧 To run these tests:"
echo ""
echo "# Test the simple approach first:"
echo "docker build -f Dockerfile.simple -t mayyam-backend:simple ."
echo ""
echo "# For Linux deployment:"
echo "docker buildx build --platform linux/amd64 -f Dockerfile.simple -t mayyam-backend:linux-amd64 --load ."
echo ""
echo "# Test the image:"
echo "docker run --rm -p 8080:8080 mayyam-backend:simple"
echo ""

echo "✅ Benefits of this approach:"
echo "- Works with regular 'docker build' on any platform"
echo "- Automatically detects architecture and chooses optimal librdkafka installation"
echo "- Fast builds on AMD64 (uses Confluent packages)"
echo "- Portable builds on ARM64 (compiles from source)"
echo "- Secure distroless runtime image (~50-80MB)"
echo "- Non-root execution"

# Check Docker and buildx availability
echo ""
echo "🔍 System Check:"
echo "Docker version: $(docker --version)"

if command -v docker-buildx >/dev/null 2>&1 || docker buildx version >/dev/null 2>&1; then
    echo "✅ Docker buildx available: $(docker buildx version)"
else
    echo "⚠️  Docker buildx not available - multi-platform builds not supported"
fi

echo ""
echo "Current architecture: $(uname -m)"
echo "Docker info:"
docker info --format "Server Version: {{.ServerVersion}}, OS/Arch: {{.ServerOSType}}/{{.Architecture}}"
