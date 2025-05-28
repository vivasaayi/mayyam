# Stage 1: Build the Rust application
FROM rust:1.87-slim as builder

WORKDIR /usr/src/app

# Install build dependencies
RUN apt-get update && \
    apt-get install -y \
        pkg-config \
        libssl-dev \
        curl \
        gnupg \
        build-essential \
        zlib1g-dev \
        cmake \
        g++ && \
    curl -sL https://packages.confluent.io/deb/7.5/archive.key | gpg --dearmor -o /usr/share/keyrings/confluent-archive-keyring.gpg && \
    echo "deb [signed-by=/usr/share/keyrings/confluent-archive-keyring.gpg] https://packages.confluent.io/deb/7.5 stable main" | tee /etc/apt/sources.list.d/confluent.list && \
    apt-get update && \
    apt-get install -y librdkafka-dev && \
    rm -rf /var/lib/apt/lists/*

# Debug: Find rdkafka.pc and its contents
RUN echo "--- [DEBUG] Post librdkafka-dev installation ---" ; \
    echo "1. Installed librdkafka-dev version details:" ; \
    apt-cache policy librdkafka-dev ; \
    echo "2. Searching for rdkafka.pc with dpkg -L:" ; \
    (dpkg -L librdkafka-dev | grep '\\.pc$' || echo "rdkafka.pc not found via dpkg -L") ; \
    echo "3. Searching for rdkafka.pc with find in /usr:" ; \
    (find /usr -name rdkafka.pc -print -exec echo "--- Contents of {} ---" \; -exec cat {} \; -exec echo "--- End of contents for {} ---" \; || echo "rdkafka.pc not found via find in /usr") ; \
    echo "4. Default pkg-config search path (PKG_CONFIG_PATH if unset):" ; \
    (pkg-config --variable pc_path pkg-config || echo "Failed to get pc_path from pkg-config") ; \
    echo "5. Default pkg-config libdir (PKG_CONFIG_LIBDIR if unset):" ; \
    (pkg-config --variable libdir pkg-config || echo "Failed to get libdir from pkg-config") ; \
    echo "--- [DEBUG] End of librdkafka-dev installation check ---"

# Set PKG_CONFIG_PATH - Assuming rdkafka.pc is in /usr/lib/x86_64-linux-gnu/pkgconfig or another standard path.
# The debug output above should confirm the location of rdkafka.pc.
# If it's different, this ENV variable might need adjustment.
ENV PKG_CONFIG_PATH_DIR="/usr/lib/x86_64-linux-gnu/pkgconfig"
ENV PKG_CONFIG_PATH="${PKG_CONFIG_PATH_DIR}${PKG_CONFIG_PATH:+:${PKG_CONFIG_PATH}}"

# Copy over the manifest files
COPY Cargo.toml .

# Create dummy source files to build dependencies
RUN mkdir src && \
    echo 'fn main() { println!("Dummy!"); }' > src/main.rs

# Build dependencies (this will be cached if dependencies don't change)
RUN echo "--- [DEBUG] Before dependency build ---" && \
    echo "1. Effective PKG_CONFIG_PATH: '${PKG_CONFIG_PATH}'" && \
    echo "2. Checking for rdkafka.pc in PKG_CONFIG_PATH_DIR (${PKG_CONFIG_PATH_DIR}):" && \
    if [ -f "${PKG_CONFIG_PATH_DIR}/rdkafka.pc" ]; then \
        echo "Found ${PKG_CONFIG_PATH_DIR}/rdkafka.pc. Contents:" && \
        cat "${PKG_CONFIG_PATH_DIR}/rdkafka.pc"; \
    else \
        echo "${PKG_CONFIG_PATH_DIR}/rdkafka.pc not found."; \
    fi && \
    echo "3. Running 'pkg-config --modversion rdkafka':" && \
    pkg-config --modversion rdkafka || echo "pkg-config --modversion rdkafka FAILED" && \
    echo "4. Running 'pkg-config --libs --cflags rdkafka':" && \
    pkg-config --libs --cflags rdkafka || echo "pkg-config --libs --cflags rdkafka FAILED" && \
    echo "5. Running 'pkg-config --validate rdkafka':" && \
    pkg-config --validate rdkafka || echo "pkg-config --validate rdkafka FAILED" && \
    echo "--- [DEBUG] Starting dependency cargo build ---" && \
    cargo build --release

# Remove the dummy source files
RUN rm -f src/main.rs

# Copy the actual source code
COPY src ./src/
COPY config.default.yml .

# Build the application
RUN echo "--- [DEBUG] Before final application build ---" && \
    echo "1. Effective PKG_CONFIG_PATH: '${PKG_CONFIG_PATH}'" && \
    echo "2. Running 'pkg-config --modversion rdkafka':" && \
    pkg-config --modversion rdkafka || echo "pkg-config --modversion rdkafka FAILED" && \
    echo "--- [DEBUG] Starting final application cargo build ---" && \
    cargo build --release

# Stage 2: Create the runtime image
FROM debian:bullseye-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates libssl1.1 curl gnupg && \
    curl -sL https://packages.confluent.io/deb/7.5/archive.key | gpg --dearmor -o /usr/share/keyrings/confluent-archive-keyring.gpg && \
    echo "deb [signed-by=/usr/share/keyrings/confluent-archive-keyring.gpg] https://packages.confluent.io/deb/7.5 stable main" | tee /etc/apt/sources.list.d/confluent.list && \
    apt-get update && \
    apt-get install -y librdkafka1 && \
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
