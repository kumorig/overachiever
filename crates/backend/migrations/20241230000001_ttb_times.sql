-- Time To Beat data from HowLongToBeat (game metadata, not user data)
CREATE TABLE ttb_times (
    appid BIGINT PRIMARY KEY,
    game_name TEXT NOT NULL,
    main REAL,
    main_extra REAL,
    completionist REAL,
    reported_count INT NOT NULL DEFAULT 1,
    first_reported_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_reported_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for batch lookups
CREATE INDEX idx_ttb_times_appid ON ttb_times(appid);
