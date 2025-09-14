#!/bin/bash

# Mayyam Development Runner
# Usage: ./dev-run.sh [backend_port] [frontend_port]

BACKEND_PORT=${1:-3000}
FRONTEND_PORT=${2:-3001}

echo "Starting Mayyam development environment..."
echo "Backend will run on port: $BACKEND_PORT"
echo "Frontend will run on port: $FRONTEND_PORT"
echo ""

# Function to run backend
run_backend() {
    echo "Starting backend on port $BACKEND_PORT..."
    cd ../backend
    cargo run -- server -p $BACKEND_PORT
}

# Function to run frontend
run_frontend() {
    echo "Starting frontend on port $FRONTEND_PORT (connecting to backend on $BACKEND_PORT)..."
    cd ../frontend
    REACT_APP_BACKEND_HOST=localhost REACT_APP_BACKEND_PORT=$BACKEND_PORT PORT=$FRONTEND_PORT npm start
}

# Run both in background
run_backend &
BACKEND_PID=$!

sleep 3  # Give backend time to start

run_frontend &
FRONTEND_PID=$!

# Wait for both processes
wait $BACKEND_PID $FRONTEND_PID
