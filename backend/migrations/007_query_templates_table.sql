-- Create query_templates table
CREATE TABLE IF NOT EXISTS query_templates (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    query TEXT NOT NULL,
    description TEXT,
    connection_type VARCHAR(50) NOT NULL,
    category VARCHAR(100),
    is_favorite BOOLEAN NOT NULL DEFAULT FALSE,
    display_order INT NOT NULL DEFAULT 999,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Add index for connection_type
CREATE INDEX idx_query_templates_connection_type ON query_templates(connection_type);

-- Add index for created_by
CREATE INDEX idx_query_templates_created_by ON query_templates(created_by);

-- Insert some default MySQL templates
INSERT INTO query_templates (
    id, name, query, description, connection_type, category, is_favorite, display_order, 
    created_by, created_at, updated_at
) VALUES (
    gen_random_uuid(), 'Show Tables', 'SHOW TABLES;', 
    'Shows all tables in the current database',
    'mysql', 'Schema', TRUE, 10,
    '00000000-0000-0000-0000-000000000001'::uuid, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
);

INSERT INTO query_templates (
    id, name, query, description, connection_type, category, is_favorite, display_order, 
    created_by, created_at, updated_at
) VALUES (
    gen_random_uuid(), 'Show Process List', 'SHOW PROCESSLIST;', 
    'Shows all running processes/connections',
    'mysql', 'Monitoring', TRUE, 20,
    '00000000-0000-0000-0000-000000000001'::uuid, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
);

INSERT INTO query_templates (
    id, name, query, description, connection_type, category, is_favorite, display_order, 
    created_by, created_at, updated_at
) VALUES (
    gen_random_uuid(), 'Top Queries by Execution Time', 
    'SELECT 
  SUBSTRING(digest_text, 1, 50) AS query_snippet,
  count_star AS exec_count,
  avg_timer_wait/1000000000 AS avg_time_ms,
  sum_timer_wait/1000000000 AS total_time_ms
FROM performance_schema.events_statements_summary_by_digest 
ORDER BY avg_timer_wait DESC 
LIMIT 10;',
    'Lists the top 10 queries by average execution time',
    'mysql', 'Performance', TRUE, 30,
    '00000000-0000-0000-0000-000000000001'::uuid, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
);

-- Insert some default PostgreSQL templates
INSERT INTO query_templates (
    id, name, query, description, connection_type, category, is_favorite, display_order, 
    created_by, created_at, updated_at
) VALUES (
    gen_random_uuid(), 'List Tables', 
    'SELECT schemaname, tablename FROM pg_tables WHERE schemaname NOT IN (''information_schema'', ''pg_catalog'');', 
    'Lists all tables in all schemas except system schemas',
    'postgresql', 'Schema', TRUE, 10,
    '00000000-0000-0000-0000-000000000001'::uuid, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
);

INSERT INTO query_templates (
    id, name, query, description, connection_type, category, is_favorite, display_order, 
    created_by, created_at, updated_at
) VALUES (
    gen_random_uuid(), 'Database Size', 
    'SELECT pg_size_pretty(pg_database_size(current_database())) AS database_size;', 
    'Shows the current database size in a human-readable format',
    'postgresql', 'Monitoring', TRUE, 20,
    '00000000-0000-0000-0000-000000000001'::uuid, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
);

INSERT INTO query_templates (
    id, name, query, description, connection_type, category, is_favorite, display_order, 
    created_by, created_at, updated_at
) VALUES (
    gen_random_uuid(), 'Active Connections', 
    'SELECT count(*) FROM pg_stat_activity WHERE state = ''active'';', 
    'Shows the number of active connections',
    'postgresql', 'Monitoring', TRUE, 30,
    '00000000-0000-0000-0000-000000000001'::uuid, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
);
