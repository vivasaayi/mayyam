Got it—reviewing your Kubernetes backend for a dashboard/manager and mapping gaps to a concrete plan.

## Checklist
- Summarize current capabilities
- Identify missing features to “perform all Kubernetes actions”
- Call out code/architecture gaps and risks
- Propose a phased implementation plan with concrete next steps

## What you already have (good base)
- Cluster registry: create/list/get/update/delete clusters stored in DB.
- Core read APIs:
  - Namespaces: list/details
  - Nodes: list/details
  - Pods: list/details/events/logs/delete
  - Services: list/details (+ list-all)
  - Deployments: list/details (+ list-all), restart, scale, delete, pods for deployment
  - StatefulSets: list/details, pods for SS (+ list-all)
  - DaemonSets: list/details, pods for DS (+ list-all)
  - PersistentVolumes: list/details
  - PersistentVolumeClaims: list/details (+ list-all)
- Logging and error mapping with AppError.
- Per-request k8s client creation via kube-rs.

## Key gaps to cover “management” and parity with kubectl/UIs
- Authentication/config
  - Not using api_server_url/token/CA/client certs in client builder (currently only kubeconfig path/infer). You have fields on KubernetesClusterConfig—wire them up.
  - No in-cluster config support toggle (job for prod).
  - No client cache; clients are rebuilt per request.

- Missing resource domains and mutating actions
  - Workloads: ReplicaSets, Jobs, CronJobs
  - Networking: Ingress, Gateway API (optional), NetworkPolicy
  - Autoscaling: HPA (VPA optional)
  - Reliability: PodDisruptionBudget
  - Config: ConfigMaps, Secrets (create/update/delete, redact on read)
  - Storage: StorageClass
  - RBAC: ServiceAccounts, Roles/ClusterRoles, RoleBindings/ClusterRoleBindings
  - Quotas: ResourceQuota, LimitRange
  - CRDs: List CRDs and browse Custom Resources (generic viewer)
  - Node ops: cordon/uncordon, drain, taints
  - Rollout: status/history/rollback (deployments/statefulsets/daemonsets)
  - Exec/attach and port-forward to pods
  - YAML view/apply/patch (server-side dry-run, strategic/merge/JSON patch)

- UX/API capabilities
  - List endpoints lack labelSelector/fieldSelector, limit/continue (pagination), sort; “all namespaces” handling should be explicit.
  - Watch streams (pods/events/deployments) for live UI via SSE/WebSocket (kube watches).
  - Logs streaming (follow/tail, container selection).
  - Standardized error codes for 401/403/404 from cluster RBAC.
  - OpenAPI/Swagger for frontend contract.

- Security and multi-tenancy
  - Do not log sensitive fields (tokens/certs). Some logs include cluster_url; ensure tokens never leak.
  - Encrypt secrets at rest (you have app encryption—use it for cluster creds).
  - Per-user cluster access, namespace scoping, and optional Kubernetes impersonation headers.
  - Audit trail of actions.

- Performance/scale
  - Client cache keyed by cluster_id/context with TTL and health checks.
  - Pagination defaults + server-enforced caps to avoid OOM on large clusters.
  - Use watch instead of heavy polling for live views.
  - Timeouts, retries, rate limiting, backpressure.

## Concrete code observations
- PodService::get_kube_client ignores api_server_url/token/CA/client certs; same in DeploymentsService. Implement a unified ClientBuilder:
  - modes: kubeconfig(path+context), direct (api_server_url + token/bearer + CA data), infer(), in_cluster().
- Clients are created per request; build a shared per-cluster client cache (DashMap<ClusterId, Client>) + eviction.
- “All namespaces” handling: some code uses empty string and others switch to Api::all only in PodService. Standardize: namespace=all => Api::all, else Api::namespaced.
- Deployments: restart/scale implemented—good. Add rollout status/history/rollback via patch to spec/template or kubectl-style annotations plus revisions.
- “Pods for deployment” in controller is currently a TODO fallback; service has a proper implementation—wire it.

## Feature matrix (high-level)
- Implement: ReplicaSets, Jobs, CronJobs, Ingress, NetworkPolicy, HPA, PDB, ConfigMap, Secret, StorageClass, RBAC objects, ResourceQuota, LimitRange, CRD/CR browser.
- Actions: exec/attach, port-forward, cordon/uncordon/drain, rollout status/history/rollback, apply/patch/dry-run, bulk delete, label/annotation editor.
- Observability: watch streams (pods, events, workloads), logs follow, basic metrics (metrics.k8s.io) and optional Prometheus integration.

## Solid plan to get there

Phase 0 — Foundations (1 week)
- Build ClientBuilder that supports: kubeconfig path/context, direct api_server_url + token/CA/client certs, infer(), in_cluster().
- Add per-cluster Client cache with TTL and health checks.
- Normalize namespace=all, labelSelector, fieldSelector, limit/continue across all list endpoints.
- Add OpenAPI spec generation (utoipa or paperclip) and serve at /api/docs.

Phase 1 — Complete read coverage (1-2 weeks)
- Add read controllers/services for: ReplicaSets, Jobs, CronJobs, Ingress, NetworkPolicy, HPA, PDB, ConfigMap, Secret (redacted), StorageClass, RBAC objects, ResourceQuota, LimitRange, CRDs and CR list/view (generic).
- Add watch endpoints (SSE/WebSocket) for pods, deployments, events (kube watch + stream to clients).
- Add logs streaming (follow, tail, container).

Phase 2 — Core mutations (1-2 weeks)
- Workloads: rollout status/history/rollback, pause/resume, scale (RS/SS/DS too).
- Pods: exec/attach (kube Api::<Pod>::exec), port-forward (Api::<Pod>::portforward).
- Node ops: cordon/uncordon/drain, taints add/remove.
- Config: CRUD for ConfigMaps/Secrets (server-side dry-run option).
- Apply YAML (server-side apply), strategic/merge/json patch.

Phase 3 — Security/Multi-tenancy (1 week)
- Encrypt cluster creds; never log secrets; secret redaction on read.
- Per-user cluster/namespace ACL and optional Kubernetes impersonation.
- Action audit logs; rate-limiting and timeouts.

Phase 4 — Observability & scale (ongoing)
- Metrics: integrate metrics.k8s.io for top pods/nodes; optional Prometheus adapter.
- Pagination defaults and max caps; backpressure for watches; cache hot lists.

Phase 5 — Polish
- E2E tests against kind/minikube; mocks for unit tests.
- Consistent error mapping and DTOs; docs and examples.

## Immediate next actions (low risk, high value)
- Implement unified ClientBuilder and cache; update services to use it.
- Standardize list endpoints: namespace (string/all), labelSelector, fieldSelector, limit, continue.
- Add exec and port-forward endpoints (needed for “management” parity).
- Add Ingress, ConfigMap, Secret, Jobs, CronJobs services/controllers.
- Add SSE watch for pods + events (unblocks live dashboard).

If you want, I can scaffold the ClientBuilder + cache and wire Pods/Deployments to use it first, then iterate resource coverage.