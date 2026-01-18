-- User-reported Time to Beat data
CREATE TABLE user_ttb_reports (
    id SERIAL PRIMARY KEY,
    steam_id BIGINT REFERENCES users(steam_id) ON DELETE CASCADE,
    appid BIGINT NOT NULL,
    main_seconds INTEGER,
    extra_seconds INTEGER,
    completionist_seconds INTEGER,
    reported_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (steam_id, appid)
);

CREATE INDEX idx_user_ttb_reports_steam_id ON user_ttb_reports(steam_id);
CREATE INDEX idx_user_ttb_reports_appid ON user_ttb_reports(appid);

-- Add aggregated average TTB fields to user_games table
ALTER TABLE user_games ADD COLUMN IF NOT EXISTS avg_user_ttb_main_seconds INTEGER;
ALTER TABLE user_games ADD COLUMN IF NOT EXISTS avg_user_ttb_extra_seconds INTEGER;
ALTER TABLE user_games ADD COLUMN IF NOT EXISTS avg_user_ttb_completionist_seconds INTEGER;
ALTER TABLE user_games ADD COLUMN IF NOT EXISTS user_ttb_report_count INTEGER NOT NULL DEFAULT 0;

-- Add user's own TTB report reference (for quick access)
ALTER TABLE user_games ADD COLUMN IF NOT EXISTS my_ttb_main_seconds INTEGER;
ALTER TABLE user_games ADD COLUMN IF NOT EXISTS my_ttb_extra_seconds INTEGER;
ALTER TABLE user_games ADD COLUMN IF NOT EXISTS my_ttb_completionist_seconds INTEGER;
ALTER TABLE user_games ADD COLUMN IF NOT EXISTS my_ttb_reported_at TIMESTAMPTZ;

-- Game-finishing achievement marker
ALTER TABLE user_achievements ADD COLUMN IF NOT EXISTS is_game_finishing BOOLEAN NOT NULL DEFAULT FALSE;

-- Create index for finding game-finishing achievements efficiently
CREATE INDEX idx_user_achievements_game_finishing ON user_achievements(steam_id, appid, is_game_finishing) WHERE is_game_finishing = TRUE;

-- Function to recalculate TTB averages for a game
CREATE OR REPLACE FUNCTION update_ttb_averages(p_appid BIGINT)
RETURNS VOID AS $$
BEGIN
    -- Update all user_games entries for this appid with the new averages
    UPDATE user_games ug
    SET 
        avg_user_ttb_main_seconds = (
            SELECT ROUND(AVG(main_seconds))::INTEGER
            FROM user_ttb_reports
            WHERE appid = p_appid AND main_seconds IS NOT NULL
        ),
        avg_user_ttb_extra_seconds = (
            SELECT ROUND(AVG(extra_seconds))::INTEGER
            FROM user_ttb_reports
            WHERE appid = p_appid AND extra_seconds IS NOT NULL
        ),
        avg_user_ttb_completionist_seconds = (
            SELECT ROUND(AVG(completionist_seconds))::INTEGER
            FROM user_ttb_reports
            WHERE appid = p_appid AND completionist_seconds IS NOT NULL
        ),
        user_ttb_report_count = (
            SELECT COUNT(*)
            FROM user_ttb_reports
            WHERE appid = p_appid
        )
    WHERE ug.appid = p_appid;
END;
$$ LANGUAGE plpgsql;

-- Trigger to update averages when a report is inserted or updated
CREATE OR REPLACE FUNCTION trigger_update_ttb_averages()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM update_ttb_averages(NEW.appid);
    
    -- Also update the user's own TTB fields in user_games
    UPDATE user_games
    SET 
        my_ttb_main_seconds = NEW.main_seconds,
        my_ttb_extra_seconds = NEW.extra_seconds,
        my_ttb_completionist_seconds = NEW.completionist_seconds,
        my_ttb_reported_at = NEW.reported_at
    WHERE steam_id = NEW.steam_id AND appid = NEW.appid;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ttb_report_changed
AFTER INSERT OR UPDATE ON user_ttb_reports
FOR EACH ROW
EXECUTE FUNCTION trigger_update_ttb_averages();

-- Trigger to update averages when a report is deleted
CREATE OR REPLACE FUNCTION trigger_update_ttb_averages_on_delete()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM update_ttb_averages(OLD.appid);
    
    -- Clear the user's own TTB fields in user_games
    UPDATE user_games
    SET 
        my_ttb_main_seconds = NULL,
        my_ttb_extra_seconds = NULL,
        my_ttb_completionist_seconds = NULL,
        my_ttb_reported_at = NULL
    WHERE steam_id = OLD.steam_id AND appid = OLD.appid;
    
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ttb_report_deleted
AFTER DELETE ON user_ttb_reports
FOR EACH ROW
EXECUTE FUNCTION trigger_update_ttb_averages_on_delete();

