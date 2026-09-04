#!/usr/bin/env bash

set -euo pipefail

repository_owner="${1:?usage: backend-base-image-ref.sh <repository-owner>}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$@"
    else
        shasum -a 256 "$@"
    fi
}

base_hash="$({
    sha256 backend/Cargo.toml \
        backend/Cargo.lock \
        backend/rust-toolchain.toml \
        Dockerfile.base
} | sha256 | awk '{print $1}')"

printf 'ghcr.io/%s/mayyam-backend-base:%s\n' "${repository_owner}" "${base_hash}"
