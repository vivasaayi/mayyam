# Kubernetes API (Mayyam)

All endpoints are served under the base path: `/api/kubernetes` and require JWT auth:

Header: `Authorization: Bearer <jwt>`

## Pod Exec

Execute a command inside a running Pod container and return stdout/stderr.

- Method: POST
- Path: `/clusters/{cluster_id}/namespaces/{namespace}/pods/{pod_name}/exec`
- Query params:
  - `command` (string, required): The command to execute. Parsed with shell-like rules (shlex). Example: `command=sh -lc "echo hello && uname -a"`
  - `container` (string, optional): Specific container name in the pod.
  - `tty` (bool, optional, default=false): Allocate a TTY.
- Body: none
- Response 200 OK:
  {
    "stdout": "...",
    "stderr": "..."
  }

Notes:
- Input is parsed using shlex; if parsing fails, the raw string is executed as a single argument.
- This endpoint currently captures stdout/stderr and returns after the command completes. Interactive stdin is disabled (stdin=false). Streaming exec/interactive sessions are planned.

## Pod Events

- Method: GET
- Path: `/clusters/{cluster_id}/namespaces/{namespace}/pods/{pod_name}/events`
- Response: JSON array of Kubernetes Event objects related to the pod.

## ConfigMaps

- GET `/clusters/{cluster_id}/namespaces/{namespace}/configmaps`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/configmaps/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/configmaps/{name}`
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/configmaps/{name}`

## Secrets

- GET (redacted) `/clusters/{cluster_id}/namespaces/{namespace}/secrets`
- GET (redacted) `/clusters/{cluster_id}/namespaces/{namespace}/secrets/{name}`
- PUT (plaintext values) `/clusters/{cluster_id}/namespaces/{namespace}/secrets/{name}`
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/secrets/{name}`

Authentication, error formats, and logging follow the conventions documented in the main README and API docs.

## Deployments

- GET `/clusters/{cluster_id}/namespaces/{namespace}/deployments` — list deployments
- GET `/clusters/{cluster_id}/namespaces/{namespace}/deployments/{name}` — get details
- POST `/clusters/{cluster_id}/namespaces/{namespace}/deployments/{name}:scale`
  - Body: `{ "replicas": <int> }`
  - Response: `{ "status": "scaled", "replicas": <int> }`
- POST `/clusters/{cluster_id}/namespaces/{namespace}/deployments/{name}:restart`
  - Response: `{ "status": "restarted" }`

## Namespaces

- GET `/clusters/{cluster_id}/namespaces` — list namespaces
- GET `/clusters/{cluster_id}/namespaces/{name}` — namespace details
- POST `/clusters/{cluster_id}/namespaces`
  - Body: `{ "name": "<ns>", "labels": { "k":"v" } }`
  - Response: Kubernetes Namespace object
- DELETE `/clusters/{cluster_id}/namespaces/{name}`
  - Response: `{ "status": "deleted", "name": "<ns>" }`
