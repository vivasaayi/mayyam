-- Sample seed SQL for integration tests
CREATE TABLE IF NOT EXISTS sample_seed (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL
);

INSERT INTO sample_seed (name) VALUES ('seed1') ON CONFLICT DO NOTHING;
