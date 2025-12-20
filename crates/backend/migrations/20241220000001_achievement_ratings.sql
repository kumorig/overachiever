-- Achievement ratings table (user ratings for individual achievements)
CREATE TABLE IF NOT EXISTS achievement_ratings (
    id SERIAL PRIMARY KEY,
    steam_id BIGINT REFERENCES users(steam_id) ON DELETE CASCADE,
    appid BIGINT NOT NULL,
    apiname TEXT NOT NULL,
    rating SMALLINT NOT NULL CHECK (rating >= 1 AND rating <= 5),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (steam_id, appid, apiname)
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_achievement_ratings_steam_id ON achievement_ratings(steam_id);
CREATE INDEX IF NOT EXISTS idx_achievement_ratings_appid_apiname ON achievement_ratings(appid, apiname);
