# Stage 1: Build the Rust application
FROM rust:1.77-slim as builder

WORKDIR /usr/src/app

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy over the manifest files
COPY Cargo.toml .

# Create dummy source files to build dependencies
RUN mkdir src && \
    echo 'fn main() { println!("Dummy!"); }' > src/main.rs

# Build dependencies (this will be cached if dependencies don't change)
RUN cargo build --release

# Remove the dummy source files
RUN rm -f src/main.rs

# Copy the actual source code
COPY src ./src/
COPY config.default.yml .

# Build the application
RUN cargo build --release

# Stage 2: Create the runtime image
FROM debian:bullseye-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates libssl1.1 && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /usr/src/app/target/release/mayyam /app/mayyam
COPY --from=builder /usr/src/app/config.default.yml /app/config.default.yml

# Set environment variables
ENV RUST_LOG=info

# Expose the API port
EXPOSE 8080

# Run the binary
CMD ["/app/mayyam", "server", "--host", "0.0.0.0", "--port", "8080"]
