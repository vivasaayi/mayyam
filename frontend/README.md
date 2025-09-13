# Frontend Development

## Running the Frontend

### Basic Commands

```bash
# Start with default configuration (backend: 3000, frontend: 3001)
npm start

# Start with specific backend port
npm run start:dev:3005    # Connects to backend on port 3005
npm run start:dev:8080    # Connects to backend on port 8080

# Start with custom ports using environment variables
BACKEND_PORT=4000 FRONTEND_PORT=3002 npm run start:custom
```

### Using the Development Script

```bash
# Run frontend only with specific backend port
./dev_frontend.sh 3005 3001    # Backend: 3005, Frontend: 3001
./dev_frontend.sh 8080         # Backend: 8080, Frontend: 3001 (default)
```

### Environment Variables

- `REACT_APP_BACKEND_HOST`: Backend hostname (default: localhost)
- `REACT_APP_BACKEND_PORT`: Backend port (default: 3000)
- `PORT`: Frontend port (default: 3001)

### Examples

```bash
# Run on different frontend port
PORT=3002 npm start

# Connect to backend on different host and port
REACT_APP_BACKEND_HOST=192.168.1.100 REACT_APP_BACKEND_PORT=8080 npm start

# Everything together
REACT_APP_BACKEND_HOST=localhost REACT_APP_BACKEND_PORT=4000 PORT=3002 npm start
```
