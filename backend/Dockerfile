# Optimized Dockerfile for Mayyam Backend
FROM rust:1.85-slim

WORKDIR /usr/src/app

# Install system dependencies including build tools for librdkafka
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        ca-certificates \
        build-essential \
        cmake \
        git \
        zlib1g-dev \
        curl && \
    rm -rf /var/lib/apt/lists/*

# Build librdkafka from source (required version >= 2.10.0)
RUN git clone --depth 1 --branch v2.10.0 https://github.com/confluentinc/librdkafka.git /tmp/librdkafka && \
    cd /tmp/librdkafka && \
    cmake -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr/local . && \
    make -j$(nproc) && \
    make install && \
    rm -rf /tmp/librdkafka

# Set PKG_CONFIG_PATH for the installed librdkafka
ENV PKG_CONFIG_PATH=/usr/local/lib/pkgconfig:$PKG_CONFIG_PATH

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create dummy src to cache dependencies
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release && rm -rf src target/release/deps/mayyam*

# Copy source code
COPY src ./src/
COPY config.default.yml config.yml ./

# Build the application
ENV RUSTFLAGS="-C opt-level=1 -C debug-assertions=off"
RUN cargo build --release

# Create non-root user
RUN useradd --create-home --shell /bin/bash appuser && \
    chown -R appuser:appuser /usr/src/app

USER appuser

# Runtime configuration
EXPOSE 8080
ENV RUST_LOG=info

CMD ["./target/release/mayyam"]
