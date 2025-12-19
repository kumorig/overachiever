-- Users table
CREATE TABLE IF NOT EXISTS users (
    steam_id BIGINT PRIMARY KEY,
    display_name TEXT NOT NULL,
    avatar_url TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_seen TIMESTAMPTZ DEFAULT NOW()
);

-- User's games (for Remote mode)
CREATE TABLE IF NOT EXISTS user_games (
    steam_id BIGINT REFERENCES users(steam_id) ON DELETE CASCADE,
    appid BIGINT NOT NULL,
    name TEXT NOT NULL,
    playtime_forever INTEGER NOT NULL DEFAULT 0,
    rtime_last_played INTEGER,
    img_icon_url TEXT,
    added_at TIMESTAMPTZ DEFAULT NOW(),
    achievements_total INTEGER,
    achievements_unlocked INTEGER,
    last_sync TIMESTAMPTZ,
    PRIMARY KEY (steam_id, appid)
);

-- User's achievements (for Remote mode)
CREATE TABLE IF NOT EXISTS user_achievements (
    steam_id BIGINT REFERENCES users(steam_id) ON DELETE CASCADE,
    appid BIGINT NOT NULL,
    apiname TEXT NOT NULL,
    achieved BOOLEAN DEFAULT FALSE,
    unlocktime TIMESTAMPTZ,
    PRIMARY KEY (steam_id, appid, apiname)
);

-- Achievement schema cache
CREATE TABLE IF NOT EXISTS achievement_schemas (
    appid BIGINT NOT NULL,
    apiname TEXT NOT NULL,
    display_name TEXT NOT NULL,
    description TEXT,
    icon TEXT NOT NULL,
    icon_gray TEXT NOT NULL,
    cached_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (appid, apiname)
);

-- Community game ratings (for Hybrid and Remote modes)
CREATE TABLE IF NOT EXISTS game_ratings (
    id SERIAL PRIMARY KEY,
    steam_id BIGINT REFERENCES users(steam_id) ON DELETE CASCADE,
    appid BIGINT NOT NULL,
    rating SMALLINT CHECK (rating >= 1 AND rating <= 5),
    comment TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (steam_id, appid)
);

-- Achievement tips (for Hybrid and Remote modes)
CREATE TABLE IF NOT EXISTS achievement_tips (
    id SERIAL PRIMARY KEY,
    steam_id BIGINT REFERENCES users(steam_id) ON DELETE CASCADE,
    appid BIGINT NOT NULL,
    apiname TEXT NOT NULL,
    difficulty SMALLINT CHECK (difficulty >= 1 AND difficulty <= 5),
    tip TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (steam_id, appid, apiname)
);

-- Sync history
CREATE TABLE IF NOT EXISTS sync_history (
    id SERIAL PRIMARY KEY,
    steam_id BIGINT REFERENCES users(steam_id) ON DELETE CASCADE,
    synced_at TIMESTAMPTZ DEFAULT NOW(),
    total_games INTEGER NOT NULL,
    total_achievements INTEGER,
    unlocked_achievements INTEGER
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_user_games_steam_id ON user_games(steam_id);
CREATE INDEX IF NOT EXISTS idx_user_achievements_steam_id ON user_achievements(steam_id);
CREATE INDEX IF NOT EXISTS idx_game_ratings_appid ON game_ratings(appid);
CREATE INDEX IF NOT EXISTS idx_achievement_tips_appid_apiname ON achievement_tips(appid, apiname);
