-- TTB Blacklist - games that should not be scanned for Time To Beat data
-- (e.g., multiplayer-only games, games without clear completion criteria)
CREATE TABLE ttb_blacklist (
    appid BIGINT PRIMARY KEY,
    game_name TEXT NOT NULL,
    reason TEXT,
    added_by_steam_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for fast lookups
CREATE INDEX idx_ttb_blacklist_appid ON ttb_blacklist(appid);
