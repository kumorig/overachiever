# TTB (Time To Beat) Integration Plan

Integrates HowLongToBeat time-to-beat data into Steam Overachiever.
**Desktop-only, hidden behind compile-time constant.**
**Data is game metadata (not user data) - stored on backend, local SQLite is cache only.**

---

## Phase 1: Foundation & Constants

- [x] **1.1** Create `crates/core/src/constants.rs` with `ENABLE_TTB: bool = true`
- [x] **1.2** Export constant from `crates/core/src/lib.rs`
- [x] **1.3** Add `TtbTimes` model struct to `crates/core/src/models.rs`:
  ```rust
  pub struct TtbTimes {
      pub appid: u64,
      pub main: Option<f32>,        // hours
      pub main_extra: Option<f32>,  // hours
      pub completionist: Option<f32>, // hours
      pub updated_at: DateTime<Utc>,
  }
  ```

---

## Phase 2: Backend Database & API

- [x] **2.1** Create migration `20241230000001_ttb_times.sql`
- [x] **2.2** Add database functions in `crates/backend/src/db.rs`
- [x] **2.3** Add API routes in `crates/backend/src/routes.rs`
- [x] **2.4** Register routes in `crates/backend/src/main.rs`

---

## Phase 3: Desktop TTB Scraper Module

- [x] **3.1** Create `crates/desktop/src/ttb/mod.rs`
- [x] **3.2** Create `crates/desktop/src/ttb/types.rs`
- [x] **3.3** Create `crates/desktop/src/ttb/scraper.rs`
- [x] **3.4** Add `mod ttb;` to `crates/desktop/src/main.rs`

---

## Phase 4: Desktop Local Cache (SQLite)

- [x] **4.1** Add TTB cache table to local SQLite in `crates/desktop/src/db.rs`
- [x] **4.2** Add cache functions (`cache_ttb_times`, `get_cached_ttb`, `get_games_without_ttb`, `get_cached_ttb_batch`)

---

## Phase 5: Desktop TTB Scan Feature

- [x] **5.1** Add TTB scan state to `crates/desktop/src/app/state.rs`:
  - `TtbScanState { current: i32, total: i32, last_fetch: Instant }`
  - 60-second delay between games

- [x] **5.2** Add scan functions to state:
  - `start_ttb_scan()` - Begin scanning games without TTB data
  - `ttb_scan_tick()` - Process one game if 60 seconds passed
  - `stop_ttb_scan()` - Cancel ongoing scan

- [x] **5.3** Add "TTB Scan" button to `crates/desktop/src/app/panels/top.rs`:
  - Next to "Full Scan" button
  - Show progress: "TTB Scan (X/Y)"
  - Only visible when `ENABLE_TTB` is true

- [x] **5.4** After successful scrape:
  1. POST to `overachiever.space/api/ttb` (share with community)
  2. Cache locally in SQLite

---

## Phase 6: Per-Game TTB Button

- [x] **6.1** Extend `GamesTablePlatform` trait in `crates/core/src/ui/games_table.rs`:
  - `fn can_fetch_ttb(&self) -> bool { false }` (default false)
  - `fn fetch_ttb(&mut self, appid: u64, game_name: &str)`
  - `fn get_ttb_times(&self, appid: u64) -> Option<&TtbTimes>`
  - `fn is_fetching_ttb(&self, appid: u64) -> bool`

- [x] **6.2** Implement trait in `crates/desktop/src/app/panels/games_table.rs`:
  - Return `ENABLE_TTB` from `can_fetch_ttb()`
  - Spawn async task to fetch TTB for single game
  - POST to backend + cache locally

- [x] **6.3** Add TTB button in expanded game row (next to Install/Launch):
  - Show clock icon button if no data
  - Show spinner if fetching
  - Immediate fetch (no rate limit for single game)

---

## Phase 7: Display TTB Times

- [x] **7.1** Add TTB display to expanded game view in `crates/core/src/ui/games_table.rs`:
  - Compact format: "Main: 12h | +Extra: 25h | 100%: 45h"
  - Only render if `ENABLE_TTB` and `can_fetch_ttb()` returns true

- [ ] **7.2** Optional: Add TTB column to game table (future enhancement)

---

## Files Summary

| File | Action | Description |
|------|--------|-------------|
| `crates/core/src/constants.rs` | Create | `ENABLE_TTB` flag |
| `crates/core/src/lib.rs` | Modify | Export constants |
| `crates/core/src/models.rs` | Modify | Add `TtbTimes` struct |
| `crates/core/src/ui/games_table.rs` | Modify | Add TTB trait methods & display |
| `crates/backend/migrations/20241230000001_ttb_times.sql` | Create | Backend table |
| `crates/backend/src/db.rs` | Modify | TTB database functions |
| `crates/backend/src/routes.rs` | Modify | TTB API endpoints |
| `crates/backend/src/main.rs` | Modify | Register TTB routes |
| `crates/desktop/src/ttb/mod.rs` | Create | Module exports |
| `crates/desktop/src/ttb/types.rs` | Create | HLTB response types |
| `crates/desktop/src/ttb/scraper.rs` | Create | HLTB scraper logic |
| `crates/desktop/src/lib.rs` | Modify | Add ttb module |
| `crates/desktop/src/db.rs` | Modify | TTB cache table/functions |
| `crates/desktop/src/app/state.rs` | Modify | TTB scan state |
| `crates/desktop/src/app/panels/top.rs` | Modify | TTB Scan button |
| `crates/desktop/src/app/panels/games_table.rs` | Modify | Implement TTB trait |

---

## Data Flow

```
Desktop App                    Backend (overachiever.space)
    |                                    |
    |  1. Check cache (SQLite)           |
    |  2. If miss, check backend ------> GET /api/ttb/{appid}
    |  3. If miss, scrape HLTB           |
    |  4. POST scraped data -----------> POST /api/ttb
    |  5. Cache locally                  |  (stores in PostgreSQL)
    |                                    |
```

---

## Progress

Started: 2024-12-30
Current Phase: ✅ Complete

**Completed:**
- Phase 1: Constants + TtbTimes model
- Phase 2: Backend DB + API endpoints (/api/ttb)
- Phase 3: Desktop scraper module (ttb/mod.rs, types.rs, scraper.rs)
- Phase 4: Local SQLite cache (ttb_cache table + functions)
- Phase 5: TTB Scan button + state management
- Phase 6: Per-game TTB button in expanded view
- Phase 7: Display TTB times in UI (7.1 done, 7.2 optional/future)

**Optional/Future:**
- Phase 7.2: Add TTB column to game table

To resume: Check boxes above, continue from first unchecked item.
