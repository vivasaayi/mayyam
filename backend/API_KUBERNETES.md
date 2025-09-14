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

## Jobs

- GET `/clusters/{cluster_id}/namespaces/{namespace}/jobs`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/jobs/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/jobs/{name}` — body is a Kubernetes Job (batch/v1)
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/jobs/{name}`

## CronJobs

- GET `/clusters/{cluster_id}/namespaces/{namespace}/cronjobs`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/cronjobs/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/cronjobs/{name}` — body is a Kubernetes CronJob (batch/v1)
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/cronjobs/{name}`

## Ingress

- GET `/clusters/{cluster_id}/namespaces/{namespace}/ingress` (alias: `/ingresses`)
- GET `/clusters/{cluster_id}/namespaces/{namespace}/ingress/{name}` (alias: `/ingresses/{name}`)
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/ingress/{name}` (alias supported) — body is networking.k8s.io/v1 Ingress
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/ingress/{name}` (alias supported)

## Endpoints and EndpointSlices

- GET `/clusters/{cluster_id}/namespaces/{namespace}/endpoints`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/endpoints/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/endpoints/{name}` — body is core/v1 Endpoints
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/endpoints/{name}`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/endpointslices` — list EndpointSlices (discovery.k8s.io/v1)

## NetworkPolicies

- GET `/clusters/{cluster_id}/namespaces/{namespace}/networkpolicies`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/networkpolicies/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/networkpolicies/{name}` — body is networking.k8s.io/v1 NetworkPolicy
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/networkpolicies/{name}`

## HorizontalPodAutoscalers (HPA)

- GET `/clusters/{cluster_id}/namespaces/{namespace}/hpa` (alias: `/horizontalpodautoscalers`)
- GET `/clusters/{cluster_id}/namespaces/{namespace}/hpa/{name}` (alias supported)
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/hpa/{name}` (alias supported) — body is autoscaling/v2 HPA
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/hpa/{name}` (alias supported)

## PodDisruptionBudgets (PDB)

- GET `/clusters/{cluster_id}/namespaces/{namespace}/poddisruptionbudgets`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/poddisruptionbudgets/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/poddisruptionbudgets/{name}` — body is policy/v1 PodDisruptionBudget
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/poddisruptionbudgets/{name}`

## ResourceQuotas

- GET `/clusters/{cluster_id}/namespaces/{namespace}/resourcequotas`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/resourcequotas/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/resourcequotas/{name}` — body is core/v1 ResourceQuota
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/resourcequotas/{name}`

## LimitRanges

- GET `/clusters/{cluster_id}/namespaces/{namespace}/limitranges`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/limitranges/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/limitranges/{name}` — body is core/v1 LimitRange
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/limitranges/{name}`

## ServiceAccounts

- GET `/clusters/{cluster_id}/namespaces/{namespace}/serviceaccounts`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/serviceaccounts/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/serviceaccounts/{name}` — body is core/v1 ServiceAccount
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/serviceaccounts/{name}`

## RBAC

Namespaced:
- GET `/clusters/{cluster_id}/namespaces/{namespace}/roles`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/roles/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/roles/{name}` — body is rbac.authorization.k8s.io/v1 Role
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/roles/{name}`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/rolebindings`
- GET `/clusters/{cluster_id}/namespaces/{namespace}/rolebindings/{name}`
- PUT `/clusters/{cluster_id}/namespaces/{namespace}/rolebindings/{name}` — body is rbac.authorization.k8s.io/v1 RoleBinding
- DELETE `/clusters/{cluster_id}/namespaces/{namespace}/rolebindings/{name}`

Cluster-scoped:
- GET `/clusters/{cluster_id}/clusterroles`
- GET `/clusters/{cluster_id}/clusterroles/{name}`
- PUT `/clusters/{cluster_id}/clusterroles/{name}` — body is rbac.authorization.k8s.io/v1 ClusterRole
- DELETE `/clusters/{cluster_id}/clusterroles/{name}`
- GET `/clusters/{cluster_id}/clusterrolebindings`
- GET `/clusters/{cluster_id}/clusterrolebindings/{name}`
- PUT `/clusters/{cluster_id}/clusterrolebindings/{name}` — body is rbac.authorization.k8s.io/v1 ClusterRoleBinding
- DELETE `/clusters/{cluster_id}/clusterrolebindings/{name}`

## Authorization (SelfSubjectAccessReview)

- POST `/clusters/{cluster_id}/authz:can`
  - Body:
    {
      "namespace": "default" | null,
      "verb": "get|list|watch|create|update|patch|delete",
      "group": "apps" | null,
      "resource": "deployments" | "pods" | ...,
      "subresource": "status" | null,
      "name": "resource-name" | null
    }
  - Response: Kubernetes SubjectAccessReviewStatus

## Node Operations

- POST `/clusters/{cluster_id}/nodes/{node}:cordon`
- POST `/clusters/{cluster_id}/nodes/{node}:uncordon`
- POST `/clusters/{cluster_id}/nodes/{node}:addTaint`
  - Body: `{ "key": "<k>", "value": "<v>", "effect": "NoSchedule|PreferNoSchedule|NoExecute" }`
- POST `/clusters/{cluster_id}/nodes/{node}:removeTaint`
  - Body: `{ "key": "<k>" }`

Notes:
- All PUT upsert endpoints accept a full Kubernetes object for the resource kind; name must match the `{name}` path.
- List endpoints support standard Kubernetes list options via query `labelSelector` and `fieldSelector` where applicable in future iterations.
