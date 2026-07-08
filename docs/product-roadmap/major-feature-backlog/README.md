# Major Feature Backlog Shards

This directory splits `../major-feature-backlog.csv` into smaller module-scoped CSV files.

Each shard uses the same columns as the consolidated major backlog and preserves `source_ids`, `source_id_start`, `source_id_end`, and `source_files` for traceability.

## Shards

| Module | File | Major items | Source rows |
| --- | --- | ---: | ---: |
| AI and LLM Observability | [ai-and-llm-observability.csv](./ai-and-llm-observability.csv) | 14 | 1078 |
| Alerting, Notification, and On-Call | [alerting-notification-and-on-call.csv](./alerting-notification-and-on-call.csv) | 19 | 1225 |
| Applications and Microservices APM | [applications-and-microservices-apm.csv](./applications-and-microservices-apm.csv) | 15 | 1127 |
| AWS Cloud | [aws-cloud.csv](./aws-cloud.csv) | 88 | 5544 |
| Azure Cloud | [azure-cloud.csv](./azure-cloud.csv) | 58 | 3654 |
| Backup, Restore, and DR Orchestrator | [backup-restore-and-dr-orchestrator.csv](./backup-restore-and-dr-orchestrator.csv) | 21 | 1225 |
| Chaos and Reliability Engineering | [chaos-and-reliability-engineering.csv](./chaos-and-reliability-engineering.csv) | 16 | 1078 |
| Dashboard and Query Workbench | [dashboard-and-query-workbench.csv](./dashboard-and-query-workbench.csv) | 15 | 1127 |
| Data Pipeline Observability | [data-pipeline-observability.csv](./data-pipeline-observability.csv) | 16 | 1127 |
| Developer Platform, SDK, and CLI | [developer-platform-sdk-and-cli.csv](./developer-platform-sdk-and-cli.csv) | 16 | 1078 |
| Digital Experience and Synthetic Checks | [digital-experience-and-synthetic-checks.csv](./digital-experience-and-synthetic-checks.csv) | 12 | 833 |
| Edge, API, and Service Mesh | [edge-api-and-service-mesh.csv](./edge-api-and-service-mesh.csv) | 16 | 1127 |
| Evidence Store and Time Machine | [evidence-store-and-time-machine.csv](./evidence-store-and-time-machine.csv) | 13 | 1176 |
| FinOps and Unit Economics | [finops-and-unit-economics.csv](./finops-and-unit-economics.csv) | 14 | 1225 |
| Fleet Management Control Plane | [fleet-management-control-plane.csv](./fleet-management-control-plane.csv) | 14 | 1176 |
| Google Cloud | [google-cloud.csv](./google-cloud.csv) | 55 | 3465 |
| IaC Drift and Change Intelligence | [iac-drift-and-change-intelligence.csv](./iac-drift-and-change-intelligence.csv) | 15 | 1323 |
| Incident Command Center | [incident-command-center.csv](./incident-command-center.csv) | 16 | 1372 |
| Kafka Dashboard and Management | [kafka-dashboard-and-management.csv](./kafka-dashboard-and-management.csv) | 18 | 1764 |
| Kubernetes Cost Allocation | [kubernetes-cost-allocation.csv](./kubernetes-cost-allocation.csv) | 12 | 980 |
| Kubernetes Dashboard | [kubernetes-dashboard.csv](./kubernetes-dashboard.csv) | 21 | 2009 |
| Learning and Runbook System | [learning-and-runbook-system.csv](./learning-and-runbook-system.csv) | 13 | 1029 |
| Linux Companion | [linux-companion.csv](./linux-companion.csv) | 41 | 3675 |
| Log Management and Analytics | [log-management-and-analytics.csv](./log-management-and-analytics.csv) | 16 | 1274 |
| MySQL AI Triager | [mysql-ai-triager.csv](./mysql-ai-triager.csv) | 17 | 1617 |
| Network Observability | [network-observability.csv](./network-observability.csv) | 20 | 1274 |
| OpenTelemetry Ingestion | [opentelemetry-ingestion.csv](./opentelemetry-ingestion.csv) | 14 | 1274 |
| Plugin and Action Marketplace | [plugin-and-action-marketplace.csv](./plugin-and-action-marketplace.csv) | 13 | 1078 |
| Postgres | [postgres.csv](./postgres.csv) | 19 | 1862 |
| RUM, Mobile Monitoring, and Session Replay | [rum-mobile-monitoring-and-session-replay.csv](./rum-mobile-monitoring-and-session-replay.csv) | 19 | 1176 |
| Secrets, Certificates, and PKI | [secrets-certificates-and-pki.csv](./secrets-certificates-and-pki.csv) | 15 | 1029 |
| Security and Compliance Packs | [security-and-compliance-packs.csv](./security-and-compliance-packs.csv) | 14 | 1176 |
| Service Catalog and Ownership | [service-catalog-and-ownership.csv](./service-catalog-and-ownership.csv) | 17 | 1372 |
| SLO and Error Budget | [slo-and-error-budget.csv](./slo-and-error-budget.csv) | 13 | 1127 |
| Telemetry Storage and Retention | [telemetry-storage-and-retention.csv](./telemetry-storage-and-retention.csv) | 16 | 1176 |
| Tenant, RBAC, and Governance | [tenant-rbac-and-governance.csv](./tenant-rbac-and-governance.csv) | 15 | 1176 |
| Universal Resource Graph | [universal-resource-graph.csv](./universal-resource-graph.csv) | 20 | 1715 |
| Workflow and Action Engine | [workflow-and-action-engine.csv](./workflow-and-action-engine.csv) | 19 | 1568 |

## Regeneration

```bash
node scripts/compress-feature-backlog.js
```
