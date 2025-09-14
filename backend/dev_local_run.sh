#!/bin/bash

# Backend development runner
# Usage: ./dev_local_run.sh [port]

PORT=3005
echo "Starting backend on port $PORT..."
cargo run -- server -p $PORT