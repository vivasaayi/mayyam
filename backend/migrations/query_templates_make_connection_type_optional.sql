-- Modify the query_templates table to make connection_type optional
ALTER TABLE query_templates 
ALTER COLUMN connection_type DROP NOT NULL;

-- Update the existing templates that should be common across all connection types
UPDATE query_templates
SET connection_type = NULL
WHERE name IN ('Show Tables', 'List Tables');

-- Add a few generic templates that work across multiple database types
INSERT INTO query_templates (
    id, name, query, description, connection_type, category, is_favorite, display_order, 
    created_by, created_at, updated_at
) VALUES (
    gen_random_uuid(), 'Show Schema Size', 
    'SELECT table_schema, 
    ROUND(SUM(data_length + index_length) / 1024 / 1024, 2) AS size_mb 
    FROM information_schema.tables 
    GROUP BY table_schema 
    ORDER BY size_mb DESC;', 
    'Shows all schemas/databases with their size in MB',
    NULL, 'Common', TRUE, 5,
    '00000000-0000-0000-0000-000000000001', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
);

INSERT INTO query_templates (
    id, name, query, description, connection_type, category, is_favorite, display_order, 
    created_by, created_at, updated_at
) VALUES (
    gen_random_uuid(), 'Count All Tables', 
    'SELECT COUNT(*) AS table_count 
    FROM information_schema.tables 
    WHERE table_schema NOT IN (''information_schema'', ''pg_catalog'', ''mysql'', ''performance_schema'', ''sys'');', 
    'Counts all user tables in the database',
    NULL, 'Common', TRUE, 6,
    '00000000-0000-0000-0000-000000000001', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
);
