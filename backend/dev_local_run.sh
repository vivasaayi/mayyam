#!/bin/bash

# Backend development runner
# Usage: ./dev_local_run.sh [port]

# Source environment variables from .env file in project root
source ../.env

echo "Starting backend on port $BACKEND_PORT..."
cargo run -- server -p $BACKEND_PORT