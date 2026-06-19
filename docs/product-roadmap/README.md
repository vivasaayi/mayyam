# Mayyam Product Roadmap

Generated on 2026-06-10 from the current repository shape, current official platform documentation, and the product goal of replacing passive observability tools with an SRE operating platform.

Mayyam should become a broad SRE/DBA/cloud engineering platform: Datadog/Dynatrace-class telemetry depth, Sentry-style investigation flow, Well-Architected governance, Linux host companionship, deterministic triage, bounded agentic investigation, and safe resource interaction across cloud, Kubernetes, databases, Kafka, and servers. This is not just observability; it is coverage, posture, diagnosis, governance, remediation, cost actionability, auditability, and executive reporting.

## Roadmap Folders

| Folder | Area | Feature Rows | Maturity |
| --- | --- | --- | --- |
| `01-aws-cloud` | AWS Cloud | 5,544 | strong partial: broad AWS inventory and data-plane foundations exist, but pillar scoring and governance are not yet unified |
| `02-kubernetes-dashboard` | Kubernetes Dashboard | 2,009 | strong partial: many resource APIs and UI tabs exist, but advanced policy, capacity, cost, release safety, and multi-cluster workflows remain |
| `03-mysql-ai-triager` | MySQL AI Triager | 1,617 | medium partial: real telemetry, deterministic signals, and LLM summary exist, but it is not yet a full DBA copilot |
| `04-kafka-dashboard-management` | Kafka Dashboard and Management | 1,764 | medium partial: cluster/topic/consumer/backup APIs exist, but observability, governance, schema, Connect, and managed Kafka support need depth |
| `05-postgres` | Postgres | 1,862 | early partial: backend analytics and prompt migration exist, but productized Postgres triage is pending |
| `06-azure-cloud` | Azure Cloud | 3,654 | greenfield pending: README says Azure is a goal, but no Azure connector surface was found in the active backend routes |
| `07-google-cloud` | Google Cloud | 3,465 | greenfield pending: no Google Cloud connector surface was found in active backend routes |
| `08-linux-companion` | Linux Companion | 3,675 | greenfield pending: no always-on Linux host companion or agent module was found in the active repository |
| `09-universal-resource-graph` | Universal Resource Graph | 1,715 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `10-service-catalog-ownership` | Service Catalog and Ownership | 1,372 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `11-incident-command-center` | Incident Command Center | 1,372 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `12-slo-error-budget` | SLO and Error Budget | 1,127 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `13-opentelemetry-ingestion` | OpenTelemetry Ingestion | 1,274 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `14-workflow-action-engine` | Workflow and Action Engine | 1,568 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `15-iac-drift-change-intelligence` | IaC Drift and Change Intelligence | 1,323 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `16-security-compliance-packs` | Security and Compliance Packs | 1,176 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `17-finops-unit-economics` | FinOps and Unit Economics | 1,225 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `18-digital-experience-synthetic-checks` | Digital Experience and Synthetic Checks | 833 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `19-plugin-action-marketplace` | Plugin and Action Marketplace | 1,078 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `20-evidence-store-time-machine` | Evidence Store and Time Machine | 1,176 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `21-learning-runbook-system` | Learning and Runbook System | 1,029 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `22-fleet-management-control-plane` | Fleet Management Control Plane | 1,176 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `23-kubernetes-cost-allocation` | Kubernetes Cost Allocation | 980 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `24-applications-microservices-apm` | Applications and Microservices APM | 1,127 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `25-ai-llm-observability` | AI and LLM Observability | 1,078 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `26-alerting-notification-oncall` | Alerting, Notification, and On-Call | 1,225 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `27-log-management-analytics` | Log Management and Analytics | 1,274 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `28-dashboard-query-workbench` | Dashboard and Query Workbench | 1,127 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `29-rum-mobile-session-replay` | RUM, Mobile Monitoring, and Session Replay | 1,176 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `30-network-observability` | Network Observability | 1,274 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `31-secrets-certificates-pki` | Secrets, Certificates, and PKI | 1,029 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `32-backup-restore-dr-orchestrator` | Backup, Restore, and DR Orchestrator | 1,225 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `33-chaos-reliability-engineering` | Chaos and Reliability Engineering | 1,078 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `34-data-pipeline-observability` | Data Pipeline Observability | 1,127 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `35-tenant-rbac-governance` | Tenant, RBAC, and Governance | 1,176 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `36-edge-api-service-mesh` | Edge, API, and Service Mesh | 1,127 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `37-developer-platform-sdk-cli` | Developer Platform, SDK, and CLI | 1,078 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |
| `38-telemetry-storage-retention` | Telemetry Storage and Retention | 1,176 | greenfield pending: platform primitive is not yet implemented as a first-class Mayyam module |

Total generated backlog rows: 59,311.

## Compressed Execution Backlog

- `major-feature-backlog.csv`: compressed delivery backlog with 785 major implementation items generated from the detailed CSV rows.
- `major-feature-backlog.md`: compression rule, module counts, and regeneration notes.
- Regenerate both files with `node scripts/compress-feature-backlog.js`.

## Foundational Docs

- `product-doctrine.md`: product positioning and the resource promise.
- `requirements-rigor.md`: definition of done, maturity levels, acceptance bar, and confirmation questions.
- `agentic-operating-model.md`: deterministic evidence, AI triage, bounded agentic investigation, and approved remediation.
- `source-module-review.md`: current code surface and architectural gaps.
- `implementation-sequencing.md`: suggested build phases.
- Every roadmap folder now includes `release-plan.md` for phase, priority, ship-size, and first-P0 execution planning.

## Product Principles

- Evidence before advice: collectors and deterministic rules must run before LLM summaries.
- Pillar-first posture: every module should report cost, resilience, performance, scalability, security, disaster recovery, and operational excellence.
- Interact, do not only observe: every resource needs inspect, diagnose, dry-run, approve, execute, rollback-note, and audit workflows where technically safe.
- Deterministic plus agentic: deterministic evaluators provide trusted facts; agentic AI investigates through bounded tools, explicit uncertainty, and approval gates.
- Cost is actionable: cost data must become savings opportunities with impact, confidence, effort, risk, owner, and verification status.
- Linux-first reach: Mayyam must work for unmanaged Linux servers, DigitalOcean droplets, VPS hosts, bare metal, cloud VMs, and Kubernetes nodes.
- Safe operations: mutations need dry-run, validation, approvals, RBAC, audit trails, and rollback notes.
- Multi-provider normalization: AWS, Azure, GCP, Kubernetes, Linux, databases, and Kafka should share identity, ownership, finding, recommendation, and workflow models.
- DBA/SRE usability: workflows should end in concrete next steps, not generic dashboards.
- Execution clarity: every module should have a release plan that makes P0 vertical slices visible without mining the full CSV.

## Source References

- AWS Well-Architected pillars: https://docs.aws.amazon.com/wellarchitected/latest/framework/the-pillars-of-the-framework.html
- AWS services by category: https://docs.aws.amazon.com/whitepapers/latest/aws-overview/amazon-web-services-cloud-platform.html
- AWS DR objectives: https://docs.aws.amazon.com/wellarchitected/latest/reliability-pillar/disaster-recovery-dr-objectives.html
- Azure Well-Architected pillars: https://learn.microsoft.com/en-us/azure/well-architected/pillars
- Azure products: https://azure.microsoft.com/en-us/products/
- Google Cloud architecture framework: https://cloud.google.com/architecture/framework
- Google Cloud products: https://cloud.google.com/products
- Kubernetes components and resource concepts: https://kubernetes.io/docs/concepts/overview/components/
- Apache Kafka introduction: https://kafka.apache.org/intro/
- PostgreSQL monitoring: https://www.postgresql.org/docs/current/monitoring.html
- Linux proc filesystem: https://docs.kernel.org/filesystems/proc.html
- systemd service manager: https://www.freedesktop.org/wiki/Software/systemd/
