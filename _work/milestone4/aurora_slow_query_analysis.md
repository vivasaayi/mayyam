
Here’s a crisp, “no-code” product spec you can hand to yourself (or a team) to build the Mayyam Aurora Slow Query Analyzer.

1) Problem definition

Teams can’t see, at a glance, which SQL patterns burn the most time and money across dozens/hundreds of Aurora (MySQL/Postgres) writers/readers.
Slow-query logs exist, but they’re noisy, hard to aggregate across clusters, and even harder to turn into actions (indexes, query fixes, or code changes). Engineers need a single, local, privacy-preserving tool that collects, normalizes, fingerprints, explains, and prioritizes SQL across all clusters—then recommends fixes with context.

Symptoms

Investigations start from anecdotes (“X is slow”) rather than data.

Repeated query patterns aren’t grouped, so impact is underestimated.

Manual digging in CloudWatch/DB logs is time-consuming and brittle.

Proposed fixes (indexes, rewrites) lack confidence and before/after proof.

Impact

Wasted infra cost (rows examined ≫ rows returned).

Longer incidents, poor SLOs, developer time sink.

Missed opportunities to tune schema and queries.

2) Vision & outcome

One pane of glass for all Aurora clusters:

Ingest slow queries at scale (hundreds of instances).

Convert raw logs → structured facts → actionable insights.

Show top offenders by total time consumed, p95 latency, and waste.

Provide Explain Plans, catalog context (tables/columns), and AI-assisted recommendations, all offline-first and private.

3) Users & jobs-to-be-done

SRE/Platform: “Which clusters and query families cause most pain this week?”

DBA: “Which indexes unlock the most benefit?” “What changed after release?”

Backend dev: “Why is my endpoint slow?” “How should I rewrite this query?”

Engineering manager: “Are we improving month over month?”

4) Non-goals (explicit)

Real-time query blocking or auto-rewrites.

Cloud-hosted analytics by default (primary mode is local/private).

Full APM replacement.

5) Data sources

Aurora MySQL: Slow query logs; optional Performance Insights for corroboration.

Aurora PostgreSQL: log_min_duration_statement; optional pg_stat_statements.

Optional: schema metadata (INFORMATION_SCHEMA / pg_catalog) for column stats, indexes.

6) Ingestion & batching (multi-cluster)
Requirements

Register N clusters (writers/readers), each with: name, engine (mysql/pg), region, log group/location, and read-only DSN for EXPLAIN.

Windowed pulls (e.g., every 10–15 minutes), with per-cluster checkpoints (last event timestamp).

Concurrency limits (max active collectors).

Backoff & resume on throttling/network faults.

De-duplication (ingest ID + timestamp + stream).

Acceptance criteria

Can pull at least 50K events/hour across ≥100 instances without drops.

Survives a 24-hour network outage and catches up without duplication.

Config change does not lose checkpoints.

7) Normalization (turn raw lines into facts)
What to extract (MySQL slow log)

Timestamps, Query_time, Lock_time, Rows_sent, Rows_examined, user@host, db, SQL text.

PostgreSQL: query duration and SQL text; associate user/db/app when available.

Acceptance criteria

99%+ of valid slow-log entries produce a structured record.

Preserve the original raw line for forensics.

8) Fingerprinting (group similar queries)
Goal

Group variants of the same query shape (literals/IDs differ) into one “fingerprint.”

Method (conceptual)

Strip numeric literals and string literals.

Normalize whitespace and keyword casing.

(Optional) parameterize IN lists / limit clauses.

Metrics per fingerprint

Count, avg/median/p95/p99 duration, total time, total rows examined vs. sent, clusters impacted, first/last seen.

Acceptance criteria

A single “get user by id” pattern with different IDs collapses to one fingerprint.

Top 20 fingerprints by total time explain ≥70% of total slow time in typical workloads.

9) Catalog extraction (tables & columns)
Goal

Let users filter/aggregate by tables, columns, joins without reading SQL.

Method (conceptual)

Parse SQL AST (SELECT/INSERT/UPDATE/DELETE).

Extract base tables, joined tables, projected columns, predicates (columns in WHERE/ORDER/GROUP), join pairs.

Acceptance criteria

For ≥90% of SELECTs, list at least one base table correctly.

Provide “top tables by total slow time” and “most-filtered columns” views.

Show missing-index candidates: columns heavily used in predicates but not indexed (needs optional schema fetch).

10) Explain plans
Capabilities

Run EXPLAIN (MySQL/PG) or EXPLAIN ANALYZE in a sandboxed/read-only manner (safe creds).

Store plan artifacts (JSON/text), with engine version & timestamp.

Highlight red flags: full table scans, filesort, temp tables, nested loops on large sets.

Workflows

From a fingerprint → pick a representative sample query → capture EXPLAIN → attach to fingerprint.

Allow before/after plan comparison (e.g., after an index is added).

Acceptance criteria

Executing EXPLAIN never mutates data.

Plans are traceable to the exact SQL/text and time window.

Side-by-side plan comparison available.

11) AI-assisted analysis (privacy-first)
Use cases

Summarize a fingerprint’s problems and likely fixes.

Suggest indexes or SQL rewrites, with rationale tied to the plan and stats.

Generate developer-friendly tickets (Jira text snippet).

Guardrails

Air-gapped / local by default; no data leaves the machine unless explicitly configured.

Pluggable providers: No-AI, local LLM, or external (OpenAI etc.) with redaction (remove literals, truncate table names if policy demands).

Acceptance criteria

AI summaries cite which evidence they used (plan nodes, p95, rows examined).

A toggle to disable AI per workspace.

12) Storage & retention (local-first)
Entities (no code; conceptual)

Cluster (id, name, engine, region, log source, DSN).

Checkpoint (cluster → last_ts).

RawEvent (cluster, ts, message, stream, ingest_id).

SlowQuery (parsed fields + sql_text).

Fingerprint (id, normalized_sql).

SlowQuery↔Fingerprint link.

CatalogRef (slow_query → table/column).

ExplainPlan (slow_query, format, payload, engine_version).

AIAnalysis (slow_query/fingerprint, model, summary, suggestions).

Retention knobs

Raw events: 7–30 days (configurable).

Parsed facts/fingerprints: 6–12 months.

Plans & AI summaries: keep until manually purged.

Acceptance criteria

Database remains responsive with 10–50 million slow-query rows (older ones can be compacted/archived).

Export to CSV/Parquet by date/fingerprint works without OOM.

13) UI/UX (your 9 bullets covered & more)

Global

Cluster picker, time window selector (Last 1h, 24h, 7d, custom).

KPI tiles: Total slow time, p95, #events, unique fingerprints, top table.

1) Download queries for a day/hour

Export dialog: choose time window, scope (all clusters / one cluster / one fingerprint).

Format: CSV (human), Parquet (bulk).

Progress with cancel/resume; writes to a chosen folder.

2) Write to a local folder

Workspace setting for default export directory.

One-click “Export this view.”

3) Parse & extract critical info

Detail panel for a single slow event: times, rows, user@host, db, SQL text.

“Why slow?” helpers: lock time vs. query time, rows examined vs. sent.

4) Fingerprint & DB storage

Fingerprint page: top by total time; each shows p50/p95, count, clusters hit.

Drill-down into samples; link to EXPLAIN if available.

5) Tables/columns extraction

“Top tables by slow time,” clickable into “columns frequently filtered.”

Badge for “suspected missing index” (predicate heavy, no index known).

6) Use AI to extract details

“Explain this fingerprint” → structured summary (root cause, proposed index, rewrite hint).

“Generate Jira text” → title, impact, steps to validate.

7) Generate EXPLAIN plans

Button on sample query: “Run EXPLAIN now” (engine-aware).

Side-by-side plan visualizer (tree view + raw JSON/text).

8) Store EXPLAIN plans

Attach plan versions to fingerprint; tag as before / after.

Badge appears when plan indicates full scan/sort/temp-table.

9) AI analysis with catalog context

The AI prompt automatically includes: normalized SQL, aggregated metrics, top plan nodes, and catalog info (tables, predicate columns, index presence).

Output is cached; re-runs only when inputs materially change.

14) Prioritization & scoring

Total time burden per fingerprint (sum of durations) — primary ranking.

Waste score: function of (rows_examined / rows_sent), table scan flags, repeated executions.

Blast radius: number of clusters/services/users affected.

Acceptance criteria: The “Top 10” page should confidently tell a DBA what to fix first this week.

15) Security & compliance

No writes to production DB except EXPLAIN/EXPLAIN ANALYZE when explicitly triggered.

Store secrets locally (keychain or encrypted file).

Role separation: different credentials for logs vs. database EXPLAIN.

PII guardrails: redaction option for literals and table names in exports/AI.

16) Performance/SLA targets (local app)

Sustain ingest of 50K–100K log lines/hour with ≤ 2 CPU cores, ≤ 4–8 GB RAM.

UI queries (top fingerprints last 24h) render in < 2 seconds on 10M rows.

Export 1M rows to CSV in < 60 seconds on SSD.

17) Observability (of the tool)

Health page: per-cluster last pull time, lag, error counts, throttling events.

Local metrics: events ingested/sec, parse failure %, queue depths.

Audit log: who ran EXPLAIN and when.

18) Edge cases & failure modes

Log storms when long_query_time is reduced: backpressure + sampling mode.

Mixed encodings / multi-line statements: robust line stitching.

DDL-heavy periods (renames): keep logical table identity via catalog snapshotting.

Timezone drift: normalize to UTC with original timezone preserved.

19) Validation & testing

Golden corpus of real slow-log lines (scrubbed) with expected parses.

Fingerprinting tests: literal variations collapse to same ID.

Plan parser tests: detect full scan/filesort reliably across versions.

“Week-over-week” synthetic dataset to verify trend views.

20) Phased delivery (sane MVP → v1 → v2)

MVP (2–3 weeks of focused build)

Multi-cluster ingestion + checkpoints.

Parse to structured rows.

Fingerprint + top offenders view.

Basic exports.

Manual EXPLAIN capture (store + view).

v1

Catalog extraction + table/column analytics.

Missing-index heuristics.

Before/after comparisons and trend charts.

Health & observability page.

v2

AI summaries with local/offline option.

Postgres parity (pg_stat_statements join).

Index simulation helper (what-if using plan hints/virtual indexes—advisory only).

Team features (annotations, shareable reports).
