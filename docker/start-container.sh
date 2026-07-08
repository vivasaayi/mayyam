#!/usr/bin/env bash
set -euo pipefail

nginx -g "daemon off;" &
nginx_pid=$!
backend_pid=""

shutdown() {
    if [[ -n "$nginx_pid" ]]; then
        kill -TERM "$nginx_pid" 2>/dev/null || true
    fi
    if [[ -n "$backend_pid" ]]; then
        kill -TERM "$backend_pid" 2>/dev/null || true
    fi
}

trap shutdown TERM INT

su appuser -s /bin/bash -c 'cd /app && LD_LIBRARY_PATH="/usr/local/lib:${LD_LIBRARY_PATH:-}" exec ./mayyam server --host 127.0.0.1 --port 8080' &
backend_pid=$!

wait -n "$nginx_pid" "$backend_pid"
status=$?

shutdown
wait "$nginx_pid" "$backend_pid" 2>/dev/null || true

exit "$status"
