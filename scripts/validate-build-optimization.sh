#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

failures=0

fail() {
    printf 'ERROR: %s\n' "$1" >&2
    failures=$((failures + 1))
}

assert_absent() {
    local pattern="$1"
    shift
    if grep -Fq -- "${pattern}" "$@"; then
        fail "unexpected '${pattern}' in $*"
    fi
}

assert_present() {
    local pattern="$1"
    local file="$2"
    if ! grep -Fq -- "${pattern}" "${file}"; then
        fail "missing '${pattern}' in ${file}"
    fi
}

# rdkafka-sys builds its bundled library through Cargo. Compiling and copying a
# second system librdkafka adds substantial work and makes the runtime less portable.
dockerfiles=(Dockerfile Dockerfile.base backend/Dockerfile.dev)
assert_absent "git clone --depth 1 --branch v2.10.0" "${dockerfiles[@]}"
assert_absent "COPY --from=backend-builder /usr/local/lib/librdkafka" Dockerfile
assert_absent "pkg-config --modversion rdkafka" "${dockerfiles[@]}"
assert_absent "PKG_CONFIG_PATH=/usr/local/lib/pkgconfig" "${dockerfiles[@]}"
assert_absent "LD_LIBRARY_PATH=/usr/local/lib" "${dockerfiles[@]}"

# PR validation must run each architecture natively. QEMU is retained only for
# the release manifest build until publishing is split into native jobs.
assert_present "runner: ubuntu-24.04-arm" .github/workflows/docker-image.yml
qemu_steps="$(grep -Fc 'uses: docker/setup-qemu-action@v3' .github/workflows/docker-image.yml || true)"
if [[ "${qemu_steps}" != "1" ]]; then
    fail "expected one release-only QEMU setup step, found ${qemu_steps}"
fi

# A push to main publishes the image and must not first repeat the PR matrix.
assert_present "if: github.event_name != 'push'" .github/workflows/docker-image.yml
assert_present "needs: [validate-compose]" .github/workflows/docker-image.yml

# Keep runtime-only nginx changes outside the React source-copy cache boundary.
assert_present "COPY docker/nginx.single-container.conf" Dockerfile
if [[ ! -f docker/nginx.single-container.conf ]]; then
    fail "docker/nginx.single-container.conf does not exist"
fi

# This unused direct dependency pins a third Smithy HTTP version into the graph.
assert_absent "aws-smithy-http =" backend/Cargo.toml

if ((failures > 0)); then
    exit 1
fi

printf 'Build optimization invariants passed.\n'
