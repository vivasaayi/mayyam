# AWS Cost Analytics - Milestone 3 Requirements

Based on the initial brainstorm and the current state of the backend implementation (Milestone 1), the following features and enhancements have been identified for the next iteration (Milestone 3).

## 1. Complete LLM Integration API
The internal foundation for generating LLM insights on cost anomalies is successfully implemented in `AwsCostAnalyticsService::generate_anomaly_insight`. However, the REST endpoint needs to be fully wired up.
- **Task:** Update `analyze_cost_with_llm` in `backend/src/controllers/cost_analytics.rs` to replace the placeholder response and actively trigger/return the LLM anomaly analysis using the backend service.
- **Task:** Add API endpoints to query and retrieve historical LLM insights from the database (e.g. `CostInsightActiveModel`).

## 2. Advanced LLM Features & UX
- **Conversational Assistant:** Chat UI/API backed by RAG to query cost data (e.g., "Why did my S3 costs increase in June?").
- **Embedding & Vector Search:** Index past reports and recommendations to provide context-aware answers.
- **Automated Summaries:** LLM-generated monthly overviews summarizing the top 5 cost drivers and suggesting immediate rightsizing/optimization actions.

## 3. Operational & Governance Features
- **Scheduled Reporting:** Automated generation and delivery of daily, weekly, or monthly cost reports via Email or Slack.
- **Budgets and Alerting:** Actively detect budget breaches and trigger alerts or open tickets.
- **Proactive Recommendations:** Actionable suggestions for Reserved Instances (RI), Compute Savings Plans (SP), EC2 rightsizing, and S3 lifecycle rules.

## 4. Multi-Account Capabilities
- Enhance cross-account reading capabilities by formalizing IAM assume-role processes for consolidated billing environments.

## 5. UI and Export Enhancements
- **Export Capabilities:** Expose the backend CSV export helper functions (`export_new_resources_csv`, `export_cost_increases_csv`, etc.) via dedicated user-facing REST endpoints for easy download.
- **Dashboard Drilldowns:** Implement dedicated drill-down pages for specific services, accounts, and custom tags.
