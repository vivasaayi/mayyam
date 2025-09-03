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

# ===== BACKEND BUILD STAGE =====
FROM rust:1.85-slim AS backend-builder

WORKDIR /usr/src/app

# Install build dependencies
RUN apt-get update && \
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
        libsasl2-dev && \
    rm -rf /var/lib/apt/lists/*

# Build librdkafka v2.10.0 from source (required for ARM64)
RUN git clone --depth 1 --branch v2.10.0 https://github.com/confluentinc/librdkafka.git /tmp/librdkafka && \
    cd /tmp/librdkafka && \
    ./configure --prefix=/usr/local && \
    make -j"$(nproc)" && \
    make install && \
    rm -rf /tmp/librdkafka

# Set PKG_CONFIG_PATH
ENV PKG_CONFIG_PATH=/usr/local/lib/pkgconfig:$PKG_CONFIG_PATH

# Verify librdkafka
RUN pkg-config --modversion rdkafka

# Copy backend Cargo files
COPY backend/Cargo.toml backend/Cargo.lock ./

# Copy backend source and build
COPY backend/src ./src/
COPY backend/config.default.yml backend/config.yml ./

RUN echo "cargo build starting"

RUN cargo build --release

RUN echo "cargo build complete"

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
