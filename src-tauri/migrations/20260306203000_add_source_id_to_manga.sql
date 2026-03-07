-- Add source_id column to manga table to support multi-source catalogs
PRAGMA foreign_keys=off;
BEGIN TRANSACTION;

-- Add new column with a sensible default 'local'
ALTER TABLE manga ADD COLUMN source_id TEXT DEFAULT 'local';

-- Ensure existing rows have a non-null source_id
UPDATE manga SET source_id = 'local' WHERE source_id IS NULL OR source_id = '';

-- Create index for fast listing by source
CREATE INDEX IF NOT EXISTS idx_manga_source_id ON manga(source_id);

COMMIT;
PRAGMA foreign_keys=on;
