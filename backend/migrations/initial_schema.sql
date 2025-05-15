-- Initial schema for Mayyam application

-- Users table for authentication and authorization
CREATE TABLE IF NOT EXISTS users (
    id VARCHAR(36) PRIMARY KEY,
    username VARCHAR(64) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    active BOOLEAN NOT NULL DEFAULT TRUE,
    roles TEXT NOT NULL DEFAULT 'user',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_login TIMESTAMP WITH TIME ZONE
);

-- Database connections table
CREATE TABLE IF NOT EXISTS database_connections (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    connection_type VARCHAR(20) NOT NULL, -- postgres, mysql, redis, opensearch
    host VARCHAR(255) NOT NULL,
    port INTEGER NOT NULL,
    username VARCHAR(100),
    password_encrypted TEXT,
    database_name VARCHAR(100),
    ssl_mode VARCHAR(20),
    cluster_mode BOOLEAN,
    created_by VARCHAR(36) NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_connected_at TIMESTAMP WITH TIME ZONE,
    connection_status VARCHAR(20)
);

-- Clusters table for Kafka, Kubernetes, and cloud providers
CREATE TABLE IF NOT EXISTS clusters (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    cluster_type VARCHAR(20) NOT NULL, -- kafka, kubernetes, aws, azure
    config JSONB NOT NULL,
    created_by VARCHAR(36) NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_connected_at TIMESTAMP WITH TIME ZONE,
    status VARCHAR(20)
);

-- Create indexes for better performance
CREATE INDEX idx_database_connections_type ON database_connections(connection_type);
CREATE INDEX idx_clusters_type ON clusters(cluster_type);
CREATE INDEX idx_users_email ON users(email);

-- Create a default admin user (password: admin123)
INSERT INTO users (
    id, 
    username, 
    email, 
    password_hash, 
    first_name, 
    last_name, 
    active, 
    roles, 
    created_at, 
    updated_at
) VALUES (
    '00000000-0000-0000-0000-000000000000',
    'admin',
    'admin@mayyam.local',
    '$argon2id$v=19$m=16,t=2,p=1$ZU55Q2pyRmJYTkZJbHJQSA$VIX6doq2qsOTWQexsl0JhA', -- hashed "admin123"
    'Admin',
    'User',
    TRUE,
    'admin,user',
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
) ON CONFLICT DO NOTHING;