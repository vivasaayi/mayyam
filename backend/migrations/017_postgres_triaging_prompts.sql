-- Migration to add PostgreSQL triaging prompt templates

-- PostgreSQL Performance Root Cause Analysis
INSERT INTO prompt_templates (id, name, description, category, prompt_type, template_content, variables) VALUES
('00000000-0000-0000-0000-000000000051', 'PostgreSQL_performance_triage', 'Root cause analysis for PostgreSQL performance issues', 'Database', 'triage', 
'You are an expert PostgreSQL Database Administrator. Analyze the following PostgreSQL performance snapshot for connection {{connection_name}} (Host: {{host}}).

Metric Context:
{{metrics_json}}

Focus your analysis on:
1. **Query Performance**: Identify slow queries using `pg_stat_statements` data.
2. **Resource Utilization**: Analyze Shared Buffers hit ratios, CPU utilization, and I/O wait.
3. **Locking & Bloat**: Check for blocked queries and potential table/index bloat (dead tuples).

Please provide:
- **Findings**: A categorized list of observations with severity.
- **Root Cause**: The most likely explanation for current performance behavior.
- **Action Items**: Concrete steps (SQL to run, VACUUM suggestions, or configuration changes) to resolve the issues.

Format your response in Markdown with clear headers.',
'{"connection_name": "string", "host": "string", "metrics_json": "object"}') ON CONFLICT (name, version) DO NOTHING;

-- PostgreSQL Connection Triage
INSERT INTO prompt_templates (id, name, description, category, prompt_type, template_content, variables) VALUES
('00000000-0000-0000-0000-000000000052', 'PostgreSQL_connection_triage', 'Analyze PostgreSQL connection spikes and session utilization', 'Database', 'triage', 
'You are an expert PostgreSQL DBA. Analyze the connection metrics for {{connection_name}}.

Connection Snapshot:
{{metrics_json}}

Analyze:
1. **Connection Limits**: Is the current connection count nearing `max_connections`?
2. **Session States**: Analyze the ratio of active vs idle sessions.
3. **Wait Events**: Identify if connections are waiting on specific locks or resources.

Provide:
- A summary of connection health.
- Recommendations for `max_connections`, `work_mem`, or implementing a connection pooler like PgBouncer.

Format your response in Markdown.',
'{"connection_name": "string", "metrics_json": "object"}') ON CONFLICT (name, version) DO NOTHING;

-- PostgreSQL Index Advisor
INSERT INTO prompt_templates (id, name, description, category, prompt_type, template_content, variables) VALUES
('00000000-0000-0000-0000-000000000053', 'PostgreSQL_index_advisor', 'Identify missing or redundant PostgreSQL indexes based on statistics', 'Database', 'analysis', 
'Analyze the following PostgreSQL table and index statistics for database {{database_name}}:

Statistics Context:
{{metrics_json}}

Provide an Index Advisory report including:
1. **Redundant/Unused Indexes**: Indexes with zero scans that consume storage.
2. **Sequential Scan Analysis**: Identify tables with high sequential scans relative to index scans.
3. **Optimization Recommendations**: Specific `CREATE INDEX` or `DROP INDEX` suggestions, including concurrent index creation where appropriate.

Format your response in Markdown.',
'{"database_name": "string", "metrics_json": "object"}') ON CONFLICT (name, version) DO NOTHING;
