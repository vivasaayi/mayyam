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
    '00000000-0000-0000-0000-000000000001',
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
