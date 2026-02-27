# Milestone 3: Kubernetes Advanced Features & Hardening

This milestone captures the deferred advanced features and security hardening requirements for the Kubernetes management module.

## 1. Unified Client & Authentication
- **Multi-Mode Client Factory**: Implement a builder that supports:
  - `kubeconfig`: Path + Context (current implementation).
  - `Direct`: Using `api_server_url`, `bearer_token`, and `CA data` (certificate) from the `KubernetesClusterConfig` database model.
  - `In-Cluster`: Support for running the agent within a pod (using ServiceAccount tokens).
- **Connection Caching**: Implement a thread-safe client cache (e.g., `DashMap<Uuid, kube::Client>`) with:
  - Time-to-Live (TTL) or health-check-based eviction.
  - Pre-fetching configurations to minimize database lookups during heavy UI usage.

## 2. Advanced Mutations & CLI Parity
- **Server-Side Apply (YAML)**:
  - Add `/apply` endpoint to support direct YAML submissions.
  - Support strategic merge patch, JSON patch, and server-side dry-runs.
- **Rollout Management**:
  - Implement `rollout status`, `history`, and `rollback` for `Deployments`, `StatefulSets`, and `DaemonSets`.
- **Pod Connectivity**:
  - **Port-Forwarding**: Implement the `port-forward` API logic to allow tunneling to pod services.
  - **Attach**: Support for attaching to running container streams.

## 3. Security & Multi-Tenancy
- **Credential Encryption**: Use the application's encryption layer to store cluster tokens and certificates encrypted at rest.
- **Impersonation**: Support Kubernetes impersonation headers (`Impersonate-User`, `Impersonate-Group`) for specialized administrative workflows.
- **Audit Logging**: Trace all mutating actions (`POST`, `PUT`, `DELETE`, `PATCH`) performed against the cluster API for compliance.

## 4. Performance & Scalability
- **Watch Backpressure**: Implement event filtering and rate limiting for SSE watch streams to avoid overwhelming the frontend or backend in large clusters.
- **Pagination Defaults**: Enforce strict `limit` and `continue` defaults across all list endpoints.
