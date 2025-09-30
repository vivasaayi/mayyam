-- Add sync_id column to aws_resources table
-- This migration adds the sync_id column that tracks which sync operation created/updated a resource

ALTER TABLE aws_resources ADD COLUMN IF NOT EXISTS sync_id UUID;

-- Add index for better query performance when filtering by sync_id
CREATE INDEX IF NOT EXISTS idx_aws_resources_sync_id ON aws_resources(sync_id);

-- Add comment for documentation
COMMENT ON COLUMN aws_resources.sync_id IS 'UUID that identifies the sync operation that created/updated this resource';