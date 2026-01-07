# Steam Tags Implementation Plan

## Overview
Add user tags from SteamSpy API with filtering and sorting. Tags are stored in backend DB (shared), fetched manually via admin mode button.

## Data Source
- **SteamSpy API**: `https://steamspy.com/api.php?request=appdetails&appid={appid}`
- Returns: `{ "tags": { "Tag Name": vote_count, ... } }`
- Rate limit: 1 request/second

## Files to Modify/Create

### 1. Backend Database Migration - OK
**New file**: `crates/backend/migrations/20250106000001_game_tags.sql`
```sql
CREATE TABLE game_tags (
    appid BIGINT NOT NULL,
    tag_name TEXT NOT NULL,
    vote_count INT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (appid, tag_name)
);
CREATE INDEX idx_game_tags_appid ON game_tags(appid);
CREATE INDEX idx_game_tags_tag_name ON game_tags(tag_name);
```

### 2. Core Models - OK
**File**: `crates/core/src/models.rs`
- Add `GameTag` struct: `{ appid, tag_name, vote_count }`

### 3. Backend Routes - OK
**File**: `crates/backend/src/routes.rs`
- Add route to get all unique tag names (for dropdown)
- Add route to get tags for specific games
- Add route to save tags (from admin fetch)

### 4. Backend DB Functions - OK
**File**: `crates/backend/src/db.rs`
- `get_all_tag_names()` - distinct tag names for dropdown
- `get_tags_for_games(appids)` - batch fetch tags
- `upsert_game_tags(appid, tags)` - save tags from SteamSpy

### 5. Desktop SteamSpy Integration - OK
**New file**: `crates/desktop/src/steamspy.rs`
- `fetch_tags(appid)` - call SteamSpy API, parse response

### 6. Desktop State - OK
**File**: `crates/desktop/src/app/mod.rs`
- Add `tags_cache: HashMap<u64, Vec<(String, u32)>>` - (tag_name, vote_count)
- Add `filter_tag: Option<String>` - currently selected tag filter
- Add `all_tags: Vec<String>` - list for dropdown
- Add `tags_fetch_queue`, `tags_receiver` etc. for async fetch

### 7. Platform Trait Extensions - OK
**File**: `crates/core/src/ui/games_table.rs`
- Add to `GamesTablePlatform` trait:
  - `filter_tag() -> Option<&str>`
  - `set_filter_tag(Option<String>)`
  - `available_tags() -> &[String]`
  - `get_tag_vote_count(appid, tag_name) -> Option<u32>`
  - `show_tags_column() -> bool` (true when tag filter active)

### 8. Desktop Platform Implementation - OK
**File**: `crates/desktop/src/app/panels/games_table.rs`
- Implement new trait methods

### 9. Filter Bar UI - OK
**File**: `crates/core/src/ui/games_table.rs` (render_filter_bar)
- Add ComboBox dropdown for tag selection
- Pattern: `egui::ComboBox::from_id_source("tag_filter")...`

### 10. Table Column - OK
**File**: `crates/core/src/ui/games_table.rs` (render_games_table)
- Add "Votes" column header (only when filter_tag is Some)
- Show vote count for selected tag in each row
- Add `SortColumn::Votes` variant

### 11. Filter Logic - OK
**File**: `crates/core/src/ui/games_table.rs` (get_filtered_indices)
- When `filter_tag` is Some, only show games that have that tag

### 12. Admin Mode Fetch Button - OK
**File**: `crates/core/src/ui/games_table.rs` (expanded row) + `crates/desktop/src/app/panels/top.rs`
- Add "Fetch Tags" button per-game (admin mode only) - in expanded row
- Add "Tags Scan" bulk button in top panel (admin mode only)
- Similar pattern to TTB fetch

### 13. Cloud Sync / Backend Calls - OK
**File**: `crates/desktop/src/cloud_sync.rs`
- Add function to fetch tags from backend
- Add function to upload fetched tags to backend

## Implementation Order

1. Backend: migration + db functions + routes
2. Core: models + trait extensions
3. Desktop: steamspy.rs + state fields
4. Desktop: platform trait implementation
5. Core UI: filter dropdown + votes column
6. Desktop: admin fetch button + async handling
7. Test end-to-end

## UI Behavior

- **Tag dropdown**: Shows "All" by default, lists all available tags
- **When tag selected**:
  - Filter shows only games with that tag
  - "Votes" column appears showing vote count
  - Can sort by votes count
- **Admin mode**: "Fetch Tags" button appears (per-game or bulk)
