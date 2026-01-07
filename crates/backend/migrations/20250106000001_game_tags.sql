-- Game tags from SteamSpy
CREATE TABLE game_tags (
    appid BIGINT NOT NULL,
    tag_name TEXT NOT NULL,
    vote_count INT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (appid, tag_name)
);

CREATE INDEX idx_game_tags_appid ON game_tags(appid);
CREATE INDEX idx_game_tags_tag_name ON game_tags(tag_name);
