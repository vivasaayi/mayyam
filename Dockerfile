# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.94
ARG SCCACHE_VERSION=0.14.0
ARG BACKEND_BASE_IMAGE=ghcr.io/sumitharajan/mayyam/mayyam-backend-base:latest

# Combined Dockerfile for Mayyam Frontend + Backend
# Multi-stage build: Frontend (React) + Backend (Rust) + Final (Nginx + API)

# ===== FRONTEND BUILD STAGE =====
FROM node:20-alpine AS frontend-builder

WORKDIR /app/frontend

# Copy frontend package files
COPY frontend/package*.json ./

# Install frontend dependencies
RUN npm ci --legacy-peer-deps

# Copy frontend source
COPY frontend/ ./

# Build the React application
RUN npm run build

# ===== BACKEND BASE FALLBACK STAGE =====
FROM rust:${RUST_VERSION}-slim AS backend-base-local

ARG TARGETARCH
ARG SCCACHE_VERSION

WORKDIR /usr/src/app

ENV PKG_CONFIG_PATH=/usr/local/lib/pkgconfig:$PKG_CONFIG_PATH \
    LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH \
    RUSTC_WRAPPER=/usr/local/cargo/bin/sccache \
    SCCACHE_DIR=/var/cache/sccache \
    SCCACHE_CACHE_SIZE=10G

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        build-essential \
        zlib1g-dev \
        curl \
        ca-certificates \
        git \
        cmake \
        g++ \
        libsasl2-dev; \
    arch="${TARGETARCH:-$(dpkg --print-architecture)}"; \
    case "${arch}" in \
        amd64|x86_64) sccache_arch="x86_64-unknown-linux-musl" ;; \
        arm64|aarch64) sccache_arch="aarch64-unknown-linux-musl" ;; \
        *) echo "Unsupported architecture: ${arch}" >&2; exit 1 ;; \
    esac; \
    curl -LsSf "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/sccache-v${SCCACHE_VERSION}-${sccache_arch}.tar.gz" | tar zxf - -C /tmp; \
    mv "/tmp/sccache-v${SCCACHE_VERSION}-${sccache_arch}/sccache" /usr/local/cargo/bin/; \
    rm -rf /tmp/sccache* /var/lib/apt/lists/*

RUN git clone --depth 1 --branch v2.10.0 https://github.com/confluentinc/librdkafka.git /tmp/librdkafka && \
    cd /tmp/librdkafka && \
    ./configure --prefix=/usr/local && \
    make -j"$(nproc)" && \
    make install && \
    rm -rf /tmp/librdkafka

RUN pkg-config --modversion rdkafka && sccache --version

COPY backend/Cargo.toml backend/Cargo.lock ./

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/var/cache/sccache,sharing=locked \
    mkdir -p src/bin && \
    echo "pub fn dummy() {}" > src/lib.rs && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/bin/hash_password.rs && \
    echo "fn main() {}" > src/bin/temp_hash.rs && \
    sccache --zero-stats && \
    cargo build --release --locked --bin mayyam && \
    (sccache --show-stats || true)

RUN rm -rf src \
           target/release/deps/mayyam* \
           target/release/mayyam* \
           target/release/.fingerprint/mayyam*

# ===== BACKEND BUILD STAGE =====
FROM ${BACKEND_BASE_IMAGE} AS backend-builder

WORKDIR /usr/src/app

COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src/
COPY backend/config.default.yml backend/config.yml ./

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/var/cache/sccache,sharing=locked \
    sccache --zero-stats && \
    cargo build --release --locked --bin mayyam && \
    (sccache --show-stats || true)

# Strip binary
RUN strip target/release/mayyam

# ===== RUNTIME STAGE =====
FROM debian:bookworm-slim

# Install nginx and runtime deps
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        nginx \
        ca-certificates \
        libssl3 \
        curl && \
    rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy backend binary and configs
COPY --from=backend-builder /usr/src/app/target/release/mayyam /app/mayyam
COPY --from=backend-builder /usr/src/app/config.default.yml /app/config.default.yml
COPY --from=backend-builder /usr/src/app/config.yml /app/config.yml

# Copy librdkafka libs
COPY --from=backend-builder /usr/local/lib/librdkafka* /usr/local/lib/

# Copy frontend build output
COPY --from=frontend-builder /app/frontend/build /usr/share/nginx/html

# Copy custom nginx config
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf

# Create non-root user for backend
RUN useradd --create-home --shell /bin/bash appuser && \
    chown -R appuser:appuser /app

# Create a startup script
RUN echo '#!/bin/bash\n\
# Start nginx in background\n\
nginx -g "daemon off;" &\n\
\n\
# Switch to appuser and start backend\n\
su - appuser -c "cd /app && LD_LIBRARY_PATH=/usr/local/lib ./mayyam server --host 127.0.0.1 --port 8080"' > /start.sh && \
    chmod +x /start.sh

# Runtime config
ENV RUST_LOG=info
ENV LD_LIBRARY_PATH=/usr/local/lib

# Expose ports
EXPOSE 80 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost/ || exit 1

# Start both services
CMD ["/start.sh"]
