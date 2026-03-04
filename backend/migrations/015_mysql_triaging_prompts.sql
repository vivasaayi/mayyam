-- Migration to add MySQL triaging prompt templates

-- MySQL Performance Root Cause Analysis
INSERT INTO prompt_templates (id, name, description, category, prompt_type, template_content, variables) VALUES
(gen_random_uuid(), 'MySQL_performance_triage', 'Root cause analysis for MySQL performance issues', 'Database', 'triage', 
'You are an expert MySQL Database Administrator. Analyze the following MySQL performance snapshot for connection {{connection_name}} (Host: {{host}}).

Metric Context:
{{metrics_json}}

Focus your analysis on:
1. **Query Performance**: Identify if slow queries or full table scans are the primary bottleneck.
2. **Resource Utilization**: Analyze Buffer Pool hit ratios, CPU wait, and IO performance.
3. **Locking & Contention**: Check for deadlocks or high row lock waits.

Please provide:
- **Findings**: A categorized list of observations with severity.
- **Root Cause**: The most likely explanation for current performance behavior.
- **Action Items**: Concrete steps (queries to run or configuration changes) to resolve the issues.

Format your response in Markdown with clear headers.',
'{"connection_name": "string", "host": "string", "metrics_json": "object"}');

-- MySQL Connection Triage
INSERT INTO prompt_templates (id, name, description, category, prompt_type, template_content, variables) VALUES
(gen_random_uuid(), 'MySQL_connection_triage', 'Analyze connection spikes and thread utilization', 'Database', 'triage', 
'You are an expert MySQL DBA. Analyze the connection metrics for {{connection_name}}.

Connection Snapshot:
{{metrics_json}}

Analyze:
1. **Connection Growth**: Is the current connection count nearing max_connections?
2. **Thread Cache Efficiency**: Is the thread cache properly configured for the workload?
3. **Aborted Connections**: Are there high rates of aborted clients or connects?

Provide:
- A summary of connection health.
- Recommendations for `max_connections`, `thread_cache_size`, or application-side connection pooling.

Format your response in Markdown.',
'{"connection_name": "string", "metrics_json": "object"}');

-- MySQL Index Advisor
INSERT INTO prompt_templates (id, name, description, category, prompt_type, template_content, variables) VALUES
(gen_random_uuid(), 'MySQL_index_advisor', 'Identify missing or redundant indexes based on table statistics', 'Database', 'analysis', 
'Analyze the following MySQL table and index statistics for database {{database_name}}:

Statistics Context:
{{metrics_json}}

Provide an Index Advisory report including:
1. **Redundant Indexes**: Indexes that are subsets of other indexes or never used.
2. **Heavy Scan Tables**: Tables with high sequential scans and large row counts that might benefit from new indexes.
3. **Optimization Recommendations**: Specific `CREATE INDEX` or `DROP INDEX` suggestions.

Format your response in Markdown.',
'{"database_name": "string", "metrics_json": "object"}');
