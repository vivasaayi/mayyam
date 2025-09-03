# Mayyam Backend Docker Optimization Guide

## Overview
The optimized Dockerfile provides:
- **Cross-platform builds**: Conditional librdkafka installation (fast packages for amd64, source build for arm64)
- **Security hardening**: Distroless runtime, non-root user, stripped binaries
- **Size optimization**: Multi-stage build, minimal runtime dependencies
- **Build caching**: Dependency layer caching, buildx cache support

## Architecture-Specific Optimizations

### Linux/AMD64 (Intel/AMD)
- Uses Confluent prebuilt librdkafka packages (v2.10.0+)
- Fastest build time (~5-10 minutes)
- Recommended for CI/CD and production

### Linux/ARM64 (Apple Silicon, ARM servers)
- Builds librdkafka from source (v2.10.0)
- Slower build time (~15-25 minutes) but portable
- Required for ARM-based deployments

## Quick Start Commands

### Local Development (Mac → Linux/AMD64)
```bash
# Build for linux/amd64 (fastest, uses Confluent packages)
./docker-build.sh --platform linux/amd64 --tag mayyam-backend:latest

# Test the image
docker run --rm -p 8080:8080 mayyam-backend:latest
```

### Multi-Architecture Build
```bash
# Build and push both amd64 and arm64
./docker-build.sh --multi-arch --tag ghcr.io/yourorg/mayyam-backend:latest --push
```

### CI/CD with Caching
```bash
# First build (creates cache)
./docker-build.sh --platform linux/amd64 --cache-dir ./.buildx-cache

# Subsequent builds (uses cache, much faster)
./docker-build.sh --platform linux/amd64 --cache-dir ./.buildx-cache
```

## Performance Benchmarks

| Platform | Build Method | Typical Time | librdkafka Source |
|----------|--------------|--------------|-------------------|
| linux/amd64 | Confluent packages | 5-8 minutes | Prebuilt (fast) |
| linux/arm64 | Source build | 15-25 minutes | Compiled (portable) |
| Multi-arch | Both methods | 20-30 minutes | Mixed |

## Security Features

### Runtime Security
- **Distroless base**: No shell, package manager, or unnecessary tools
- **Non-root user**: Runs as `nonroot:nonroot` (UID 65532)
- **Minimal attack surface**: Only essential libraries and the application binary

### Build Security
- **Signed packages**: Uses GPG-verified Confluent repository keys
- **No secrets in image**: Build-time only credentials and keys
- **Stripped binaries**: Debug symbols removed for smaller size

## Size Optimization

### Final Image Size
- **Typical size**: 50-80 MB (vs 500+ MB with full Debian base)
- **Components**:
  - Distroless base: ~20 MB
  - Rust binary (stripped): ~15-30 MB
  - librdkafka + dependencies: ~10-20 MB
  - Config files: <1 MB

### Multi-Stage Benefits
1. **Builder stage**: Contains all build tools (~1-2 GB during build)
2. **Runtime stage**: Only essential runtime files (~50-80 MB final)
3. **Clean separation**: No build artifacts in final image

## Troubleshooting

### Common Issues

#### Build fails with "rdkafka not found"
```bash
# Check PKG_CONFIG_PATH in build logs
docker buildx build --progress=plain --platform linux/amd64 .
```

#### Multi-arch build too slow
```bash
# Build architectures separately
./docker-build.sh --platform linux/amd64 --tag myapp:amd64
./docker-build.sh --platform linux/arm64 --tag myapp:arm64

# Create manifest
docker manifest create myapp:latest myapp:amd64 myapp:arm64
docker manifest push myapp:latest
```

#### Runtime library missing
```bash
# For source-built librdkafka, ensure .so files are copied
# Check Dockerfile COPY --from=builder /usr/local/lib/librdkafka*.so* line
```

### Performance Tuning

#### Faster Builds
1. **Use cache mounts**: Already implemented with `--cache-dir`
2. **Separate dependency layer**: Done with dummy main.rs trick
3. **Parallel compilation**: `make -j"$(nproc)"` for librdkafka
4. **Registry caching**: Push/pull intermediate layers

#### Smaller Images
1. **Static linking**: Consider `musl` target for fully static binary
2. **UPX compression**: Compress binary (may affect performance)
3. **Minimal config**: Remove unnecessary config files

## Advanced Configuration

### Custom librdkafka Version
Edit Dockerfile line 32 to change version:
```dockerfile
git clone --depth 1 --branch v2.11.0 https://github.com/confluentinc/librdkafka.git
```

### Different Base Images
- **scratch**: For static binaries (smallest)
- **distroless/static**: For Go-style static binaries
- **alpine**: Small but with shell (debugging)
- **debian:bullseye-slim**: Full featured (largest)

### Build Arguments
```bash
# Custom build args
docker buildx build \
  --build-arg RUST_VERSION=1.85 \
  --build-arg LIBRDKAFKA_VERSION=v2.10.0 \
  --platform linux/amd64 \
  .
```

## CI/CD Integration

### GitHub Actions Example
```yaml
- name: Build and push Docker image
  run: |
    docker buildx create --use
    ./docker-build.sh \
      --multi-arch \
      --tag ghcr.io/${{ github.repository }}:${{ github.sha }} \
      --push \
      --cache-dir /tmp/.buildx-cache
```

### Local Registry Testing
```bash
# Run local registry
docker run -d -p 5000:5000 --name registry registry:2

# Build and push locally
./docker-build.sh \
  --tag localhost:5000/mayyam-backend:test \
  --push

# Pull and test
docker pull localhost:5000/mayyam-backend:test
```
