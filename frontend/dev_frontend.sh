#!/bin/bash

# Frontend development runner
# Usage: ./dev_frontend.sh [backend_port] [frontend_port]

source ../.env

echo "Starting frontend on port $FRONTEND_PORT..."
echo "Connecting to backend on port $BACKEND_PORT..."


REACT_APP_BACKEND_PORT=$BACKEND_PORT REACT_APP_API_URL=http://localhost:$BACKEND_PORT PORT=$FRONTEND_PORT npm start
