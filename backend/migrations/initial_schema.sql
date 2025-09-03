-- Initial schema for Mayyam application

-- Users table for authentication and authorization
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
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
    id UUID PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    connection_type VARCHAR(20) NOT NULL, -- postgres, mysql, redis, opensearch
    host VARCHAR(255) NOT NULL,
    port INTEGER NOT NULL,
    username VARCHAR(100),
    password_encrypted TEXT,
    database_name VARCHAR(100),
    ssl_mode VARCHAR(20),
    cluster_mode BOOLEAN,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_connected_at TIMESTAMP WITH TIME ZONE,
    connection_status VARCHAR(20)
);

-- Clusters table for Kafka, Kubernetes, and cloud providers
CREATE TABLE IF NOT EXISTS clusters (
    id UUID PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    cluster_type VARCHAR(20) NOT NULL, -- kafka, kubernetes, aws, azure
    config JSONB NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_connected_at TIMESTAMP WITH TIME ZONE,
    status VARCHAR(20)
);

-- Create indexes for better performance
CREATE INDEX idx_database_connections_type ON database_connections(connection_type);
CREATE INDEX idx_clusters_type ON clusters(cluster_type);
CREATE INDEX idx_users_email ON users(email);

-- Setup initial user for Mayyam application

-- Create a default admin user (password: admin123)
-- Using the correct schema from the database
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
    '00000000-0000-0000-0000-000000000001'::uuid,
    'admin',
    'admin@mayyam.local',
    '$2b$10$oz69QfHeT6BhqP3Gl5qzFuBBUZYqb1xKJv6Kciykra9983.qBLsse', -- bcrypt hash for "admin123"
    'Admin',
    'User',
    TRUE,
    'admin,user',
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
) ON CONFLICT (username) DO NOTHING;