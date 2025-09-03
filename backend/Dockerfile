# Final Working Dockerfile - Simple and Fast
FROM rust:1.85-slim

WORKDIR /usr/src/app

# Install dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        librdkafka-dev \
        ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy only necessary files
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY config.default.yml config.yml ./

# Build with faster settings
ENV RUSTFLAGS="-C opt-level=1 -C debug-assertions=off"
RUN cargo build --release

# Create user and set permissions
RUN useradd --create-home --shell /bin/bash appuser && \
    chown -R appuser:appuser /usr/src/app

USER appuser

# Runtime
EXPOSE 8080
CMD ["./target/release/mayyam"]

# Verify librdkafka installation
RUN pkg-config --modversion rdkafka

# Copy dependency files and cache dependency builds
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source and build the application
COPY src ./src/
COPY config.default.yml .
COPY config.yml .

RUN cargo build --release

# Strip debug symbols for smaller binary
RUN apt-get update && \
    apt-get install -y --no-install-recommends binutils && \
    strip target/release/mayyam && \
    rm -rf /var/lib/apt/lists/*

# Stage 2: Runtime image - minimal and secure
FROM gcr.io/distroless/cc-debian11:latest

WORKDIR /app

# Create non-root user (distroless already has nonroot user)
USER nonroot:nonroot

# Copy the stripped binary
COPY --from=builder --chown=nonroot:nonroot /usr/src/app/target/release/mayyam /app/mayyam

# Copy configuration files
COPY --from=builder --chown=nonroot:nonroot /usr/src/app/config.default.yml /app/config.default.yml
COPY --from=builder --chown=nonroot:nonroot /usr/src/app/config.yml /app/config.yml

# Copy required shared libraries if librdkafka was built from source
COPY --from=builder /usr/local/lib/librdkafka*.so* /usr/local/lib/

# Set environment variables
ENV RUST_LOG=info
ENV PKG_CONFIG_PATH=/usr/local/lib/pkgconfig

# Expose the API port
EXPOSE 8080

# Run the binary
ENTRYPOINT ["/app/mayyam"]
CMD ["server", "--host", "0.0.0.0", "--port", "8080"]
