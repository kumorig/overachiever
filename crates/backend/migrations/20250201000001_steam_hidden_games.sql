-- Add steam_hidden column to user_games table (separate from manual hidden)
ALTER TABLE user_games ADD COLUMN IF NOT EXISTS steam_hidden BOOLEAN DEFAULT FALSE;

-- Index for filtering steam-hidden games
CREATE INDEX IF NOT EXISTS idx_user_games_steam_hidden ON user_games(steam_id, steam_hidden);

-- Grant permissions to overachiever user
GRANT SELECT, INSERT, UPDATE, DELETE ON user_games TO overachiever;
