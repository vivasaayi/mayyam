i• Outcome: the production image now builds and runs on this Mac/Apple Silicon Docker setup, and the distributable compose smoke is healthy.

  Repo shape:

  - backend/: Rust Actix backend.
  - frontend/: React/CoreUI frontend.
  - Root Dockerfile: single production image, frontend + backend + nginx.
  - Dockerfile.base: reusable Rust dependency/base image.
  - docker-compose.distributable.yml: user-facing compose bootstrap.
  - .github/workflows/docker-*.yml: CI image/base-image builds.

  What I fixed:

  - Included backend/migrations in the Docker build so embedded migrations compile.
  - Pinned Rust builder to rust:1.94-slim-bookworm to match the debian:bookworm-slim runtime.
  - Capped Cargo jobs with CARGO_BUILD_JOBS=4 to avoid Mac Docker/sccache instability.
  - Added runtime libs and a proper /start.sh for nginx + backend.
  - Added a single-container nginx config and removed Debian’s default nginx site.
  - Updated compose health checks to use /health.
  - Updated GitHub Actions to smoke-build the production image and publish multi-arch images.
  - Added the one-line install path in docs/README:
    curl -fsSL https://raw.githubusercontent.com/sumitharajan/mayyam/main/scripts/install.sh | bash

  Validated:

  - docker build -t mayyam-container-check . passed on Mac ARM.
  - First full local build was slow: Rust dependency layer took ~27m45s, app compile ~6m13s.
  - docker run --rm --entrypoint /app/mayyam mayyam-container-check --help passed.
  - MAYYAM_IMAGE=mayyam-container-check docker compose --env-file .env.distributable -f docker-compose.distributable.yml -p mayyam-smoke up -d passed.
  - Smoke stack is healthy; curl -fsS http://127.0.0.1/health returns Mayyam API is running and DBs are healthy.
  - Frontend root returns HTTP 200.
  - bash -n, compose config, and workflow YAML validation passed.

  I left the isolated smoke stack running at http://127.0.0.1. Stop it with:

  docker compose --env-file .env.distributable -f docker-compose.distributable.yml -p mayyam-smoke down

  Caveats: Linux/amd64 was not run locally here, but Actions are now set up for linux/amd64 and linux/arm64. Also, your existing non-smoke local stack had unhealthy/
  restarting containers before this check. Separate security note: the local git remote appears to include a GitHub token; rotate/remove that token from the remote
  URL.


