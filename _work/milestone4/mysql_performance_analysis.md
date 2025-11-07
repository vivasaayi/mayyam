# ✅ Mayyam – MySQL Performance Analyzer (with AI Insights)

## 1. 🎯 Problem Statement

Developers and SRE teams struggle to:
- Quickly understand what is slowing down a MySQL/Aurora database.
- Identify expensive queries, missing indexes, misconfigurations.
- Analyze performance across multiple clusters manually.
- Convert raw performance metrics into clear, actionable insights.

**Goal:**  
Build a private, offline-first performance dashboard (like MySQL Workbench Performance) but smarter—with:
✔ AI insights  
✔ Manual health checks  
✔ Slow query investigation  
✔ Explain plan storage  
✔ Trends and root cause analysis  

---

## 2. ✅ Core Capabilities

### **A. Manual Health Check (One-click Analysis)**
Runs a real-time snapshot across metrics:
| Area | Includes |
|------|----------|
| Workload | QPS/TPS, threads running, connections, read/write ratio |
| Slow Queries | Top SQL patterns, total execution time, p95 latency |
| Wait Events | Locking, I/O, CPU, metadata locks |
| InnoDB Engine | Buffer pool, log file usage, history length, flushes |
| Temporary Tables | Disk-usage vs memory temp tables |
| Replication Status | Lag, SQL/IO thread metrics |
| Config Validation | innodb_buffer_pool_size, redo log size, query cache, etc |

**Output:**  
- Health Score (A/B/C/D)  
- “Top 5 Issues” + suggested fixes  
- Downloadable report (Markdown / PDF)

---

### **B. Slow Query Analyzer**
- List all slow queries from `performance_schema` / slow query logs  
- Group by **fingerprint** (parameterized SQL)  
- Metrics:
  - p50/p95/p99 latency  
  - Total execution time  
  - Rows examined vs rows returned  
  - Frequency & clusters affected  

---

### **C. Query Drilldown**
For each normalized SQL fingerprint:
| Data | Description |
|------|-------------|
| Sample SQL | Real query from logs |
| Metrics | Avg/Max/95th percentile time |
| Row Analysis | Rows examined vs sent |
| Tables Used | Extracted using SQL parsing |
| EXPLAIN Plans | Stored JSON/Text plans |
| AI Summary | “What’s wrong & how to fix it?” |

---

### **D. Schema & Index Analysis**
| Feature | Purpose |
|---------|---------|
| Missing Indexes | Queries filtering on columns without indexes |
| Unused Indexes | Indexes never used (from `performance_schema`) |
| Duplicate Indexes | Redundant or overlapping indexes |
| Large Tables | Sorted by storage size / scans |
| Column Analysis | Data type, null count, index participation |

---

### **E. Lock & Wait Diagnostics**
- Current blocking queries  
- Deadlock reports  
- Who is blocking who (query + user + table)
- InnoDB row lock time stats  
- Metadata locks (DDL issues)

---

### **F. Explain Plan Management**
| Capability | Description |
|------------|-------------|
| Run EXPLAIN / EXPLAIN ANALYZE | On-demand or batch |
| Store Plans Locally | With fingerprint reference |
| Before vs After Comparison | Track improvements |
| Highlights | Full table scan, filesort, temp tables, missing indexes |

---

### **G. AI-Powered Insights (Optional)**
- Uses sanitized SQL + metrics  
- Provides:
  - Root cause of slow performance  
  - Suggested indexes  
  - Suggested SQL rewrites  
  - Configuration improvements  
- Generates:
  - **Jira-ready ticket:** “Add index on orders(user_id)”  
  - **Release Note Summary:** “p95 latency dropped after indexing”

---

## 3. 🗂 Data Sources (Read-Only)

| Source | Usage |
|--------|-------|
| performance_schema.events_statements_summary_by_digest | Query stats |
| information_schema.tables / columns | Table metadata |
| sys schema views | Simplified performance insights |
| SHOW GLOBAL STATUS | Server runtime metrics |
| SHOW ENGINE INNODB STATUS | Buffer pool, history length |
| performance_schema.events_waits | Lock & I/O waits |

---

## 4. 📊 UI Overview

Page | Features
-----|---------
**Dashboard** | Health Score, QPS, p95 latency, Buffer Pool %, Temp on Disk %
**Slow Queries** | Top query fingerprints with stats
**Query Details** | Full SQL, EXPLAIN, table usage, AI summary
**Schema & Indexes** | Missing/unused indexes, hot tables
**Locks & Waits** | Blocking sessions, deadlocks, locks per table
**Config Check** | my.cnf variable warnings + best practices
**Reports / Export** | Export CSV/PDF of findings
**AI Workspace** | “Ask AI why this query is slow”

---

## 5. 🧠 AI Workflow Design

**Input AI Receives:**
Normalized SQL

p95 latency, total exec time

Rows examined/sent

Explain plan summary

Table/index structures

yaml
Copy code

**AI Output:**
- Root cause explanation  
- Index recommendation  
- Query rewrite suggestion  
- Validation steps  
- Impact estimation  

All AI usage is optional and can be:
✔ Disabled (Fully Offline Mode)  
✔ Local LLama model  
✔ OpenAI with sanitization  

---

## 6. ✅ Non-Goals

- Real-time APM like Datadog
- Query tuning automation (no auto-index apply)
- Cloud-hosted dashboards (local-first approach)

---

## 7. ✅ Success Criteria

✔ Health check report in **< 1 minute**  
✔ Identify **Top 10 problem queries** that cause >70% of slow time  
✔ After applying suggested fixes, p95 improves by **>30%**  
✔ Query + Explain + AI summary exported in a sharable format (PDF/CSV)

---

## 8. 🚀 Future Enhancements

- PostgreSQL Performance integration  
- Workload classification (OLTP vs OLAP)  
- Query replay / simulation engine  
- MySQL vs Aurora comparison for migration planning  
- Slack/Webhook alerts: “p95 went above threshold”  

---
