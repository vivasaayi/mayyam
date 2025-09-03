#!/bin/bash
# Docker Build Debug and Fix Script

echo "🔍 Docker Build Debugging Guide"
echo "==============================="

echo ""
echo "📊 Current Docker status:"
docker info --format "Version: {{.ServerVersion}}, OS: {{.ServerOSType}}/{{.Architecture}}"
echo "Available memory for Docker:"
docker system df

echo ""
echo "🚫 Common issues causing stuck builds:"
echo "1. Large context size (target/ directory)"
echo "2. Too many parallel jobs (memory exhaustion)"
echo "3. Network timeouts during package downloads"
echo "4. Complex dependency compilation"

echo ""
echo "✅ Quick fixes to try:"

echo ""
echo "🧹 1. Clean up Docker to free memory:"
echo "docker system prune -a --volumes"
echo "docker builder prune"

echo ""
echo "⚡ 2. Use the fast Dockerfile:"
echo "docker build -f Dockerfile.final -t mayyam-backend:latest ."

echo ""
echo "🔧 3. Build with resource limits:"
echo "docker build --memory=4g --cpus=2 -f Dockerfile.final -t mayyam-backend:latest ."

echo ""
echo "📦 4. Try multi-stage minimal build:"
cat << 'EOF'
# Create Dockerfile.ultra-minimal:
FROM rust:1.85-slim as builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev librdkafka-dev
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY config*.yml ./
RUN cargo build --release --bin mayyam

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y librdkafka1 libssl1.1 ca-certificates
COPY --from=builder /app/target/release/mayyam /usr/local/bin/
COPY --from=builder /app/config*.yml /app/
WORKDIR /app
USER 1000:1000
CMD ["mayyam", "server", "--host", "0.0.0.0", "--port", "8080"]
EOF

echo ""
echo "🏃‍♂️ 5. Local cargo build first (fastest for testing):"
echo "cargo build --release"
echo "# Then test the binary locally before Docker"

echo ""
echo "⏱️ 6. Monitor build progress:"
echo "docker build --progress=plain -f Dockerfile.final -t mayyam-backend:latest . 2>&1 | tee build.log"

echo ""
echo "🛑 If build is currently stuck, kill it:"
echo "docker ps -q | xargs docker kill 2>/dev/null || true"

echo ""
echo "Current build status check:"
if docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Command}}" | grep -q build; then
    echo "⚠️  Build containers still running:"
    docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Command}}" | grep build
else
    echo "✅ No build containers running"
fi

echo ""
echo "💾 Disk space check:"
df -h /Users/$USER/Library/Containers/com.docker.docker 2>/dev/null || echo "Docker disk usage: Use Docker Desktop GUI"
