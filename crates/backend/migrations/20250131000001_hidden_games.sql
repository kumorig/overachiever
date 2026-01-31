-- Add hidden column to user_games table
ALTER TABLE user_games ADD COLUMN IF NOT EXISTS hidden BOOLEAN DEFAULT FALSE;

-- Index for filtering hidden games
CREATE INDEX IF NOT EXISTS idx_user_games_hidden ON user_games(steam_id, hidden);

-- Grant permissions to overachiever user
GRANT SELECT, INSERT, UPDATE, DELETE ON user_games TO overachiever;
