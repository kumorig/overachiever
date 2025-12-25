-- Add short_id column for shareable profile URLs (YouTube-style short IDs)
ALTER TABLE users ADD COLUMN IF NOT EXISTS short_id VARCHAR(12) UNIQUE;

-- Create index for fast lookups
CREATE INDEX IF NOT EXISTS idx_users_short_id ON users(short_id);
