-- Add unplayed_games_total column to run_history table
-- This stores total unplayed games regardless of achievements
-- (unplayed_games stores only unplayed games WITH achievements)
ALTER TABLE run_history ADD COLUMN IF NOT EXISTS unplayed_games_total INTEGER NOT NULL DEFAULT 0;
