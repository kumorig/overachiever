use crate::config::Config;
use overachiever_core::{Game, SteamGame, Achievement, AchievementSchema};
use std::sync::mpsc::Sender;

const API_OWNED_GAMES: &str = "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";
const API_RECENTLY_PLAYED: &str = "https://api.steampowered.com/IPlayerService/GetRecentlyPlayedGames/v1/";
const API_ACHIEVEMENTS: &str = "http://api.steampowered.com/ISteamUserStats/GetPlayerAchievements/v0001/";
const API_SCHEMA: &str = "http://api.steampowered.com/ISteamUserStats/GetSchemaForGame/v2/";

#[derive(Clone)]
pub enum FetchProgress {
    Requesting,
    Downloading,
    Processing,
    Saving,
    Done { games: Vec<Game>, total: i32 },
    Error(String),
}

#[derive(Clone)]
pub enum ScrapeProgress {
    FetchingGames,
    Starting { total: i32 },
    Scraping { current: i32, total: i32, game_name: String },
    GameUpdated { appid: u64, unlocked: i32, total: i32 },
    Done { games: Vec<Game> },
    Error(String),
}

#[derive(Clone)]
pub enum UpdateProgress {
    FetchingGames,
    FetchingRecentlyPlayed,
    ScrapingAchievements { current: i32, total: i32, game_name: String },
    GameUpdated { appid: u64, unlocked: i32, total: i32 },
    Done { games: Vec<Game>, updated_count: i32 },
    Error(String),
}

#[derive(Clone)]
pub enum SingleGameRefreshProgress {
    Refreshing { appid: u64 },
    Done { 
        appid: u64, 
        game: Game,
        achievements: Vec<overachiever_core::GameAchievement>,
    },
    Error(String),
}

pub fn fetch_owned_games_with_progress(progress_tx: Sender<FetchProgress>) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load();
    if !config.has_steam_credentials() {
        let _ = progress_tx.send(FetchProgress::Error("Please configure steam_web_api_key and steam_id in config.toml".to_string()));
        return Ok(());
    }
    let steam_key = &config.steam_web_api_key;
    let steam_id = config.steam_id_u64().unwrap();
    
    let input = serde_json::json!({
        "steamid": steam_id,
        "include_appinfo": 1,
        "include_played_free_games": 1
    });
    
    let url = format!(
        "{}?key={}&input_json={}&format=json",
        API_OWNED_GAMES,
        steam_key,
        urlencoding::encode(&input.to_string())
    );
    
    // Stage 1: Requesting
    let _ = progress_tx.send(FetchProgress::Requesting);
    
    let response = reqwest::blocking::get(&url)?;
    
    // Stage 2: Downloading
    let _ = progress_tx.send(FetchProgress::Downloading);
    
    let body_text = response.text()?;
    
    // Stage 3: Processing
    let _ = progress_tx.send(FetchProgress::Processing);
    
    let body: serde_json::Value = serde_json::from_str(&body_text)?;
    
    let games: Vec<SteamGame> = body["response"]["games"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|g| serde_json::from_value(g.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    
    // Stage 4: Saving to database
    let _ = progress_tx.send(FetchProgress::Saving);
    
    let total = games.len() as i32;
    let unplayed = games.iter().filter(|g| g.playtime_forever == 0).count() as i32;
    let conn = crate::db::open_connection()?;
    crate::db::upsert_games(&conn, &config.steam_id, &games)?;
    crate::db::insert_run_history(&conn, &config.steam_id, total, unplayed)?;
    
    // Stage 5: Done - reload from DB to get consistent state
    let games = crate::db::get_all_games(&conn, &config.steam_id)?;
    let _ = progress_tx.send(FetchProgress::Done { games, total });
    
    Ok(())
}

pub fn scrape_achievements_with_progress(progress_tx: Sender<ScrapeProgress>, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load();
    if !config.has_steam_credentials() {
        let _ = progress_tx.send(ScrapeProgress::Error("Please configure steam_web_api_key and steam_id in config.toml".to_string()));
        return Ok(());
    }
    let steam_key = &config.steam_web_api_key;
    let steam_id = config.steam_id_u64().unwrap();
    
    // Step 1: Fetch games first
    let _ = progress_tx.send(ScrapeProgress::FetchingGames);
    
    let input = serde_json::json!({
        "steamid": steam_id,
        "include_appinfo": 1,
        "include_played_free_games": 1
    });
    
    let url = format!(
        "{}?key={}&input_json={}&format=json",
        API_OWNED_GAMES,
        steam_key,
        urlencoding::encode(&input.to_string())
    );
    
    let response = reqwest::blocking::get(&url)?;
    let body: serde_json::Value = response.json()?;
    
    let games: Vec<SteamGame> = body["response"]["games"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|g| serde_json::from_value(g.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    
    let conn = crate::db::open_connection()?;
    crate::db::upsert_games(&conn, &config.steam_id, &games)?;
    let total_games = games.len() as i32;
    let unplayed_games = games.iter().filter(|g| g.playtime_forever == 0).count() as i32;
    crate::db::insert_run_history(&conn, &config.steam_id, total_games, unplayed_games)?;

    // Step 1.5: Fetch recently played games (to capture F2P games not in GetOwnedGames)
    let recent_games = fetch_recently_played_games(steam_key, steam_id, config.debug_recently_played)?;
    if !recent_games.is_empty() {
        crate::db::upsert_games(&conn, &config.steam_id, &recent_games)?;

        // Recalculate total games after adding recently played
        let all_games_after_upsert = crate::db::get_all_games(&conn, &config.steam_id)?;
        let new_total = all_games_after_upsert.len() as i32;
        if new_total > total_games {
            crate::db::update_run_history_total(&conn, &config.steam_id, new_total)?;
        }
    }

    // Step 2: Scrape achievements - either just unscraped games or all games if force is true
    let games_to_scrape = if force {
        crate::db::get_all_games(&conn, &config.steam_id)?
    } else {
        crate::db::get_games_needing_achievement_scrape(&conn, &config.steam_id)?
    };
    let total = games_to_scrape.len() as i32;
    
    let _ = progress_tx.send(ScrapeProgress::Starting { total });
    
    for (i, game) in games_to_scrape.iter().enumerate() {
        let _ = progress_tx.send(ScrapeProgress::Scraping {
            current: i as i32 + 1,
            total,
            game_name: game.name.clone(),
        });
        
        // Fetch player achievements
        let url = format!(
            "{}?appid={}&key={}&steamid={}&format=json",
            API_ACHIEVEMENTS,
            game.appid,
            steam_key,
            steam_id
        );
        
        match reqwest::blocking::get(&url) {
            Ok(response) => {
                if let Ok(body) = response.text() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(achievements_arr) = json["playerstats"]["achievements"].as_array() {
                            let achievements: Vec<Achievement> = achievements_arr
                                .iter()
                                .filter_map(|a| serde_json::from_value(a.clone()).ok())
                                .collect();
                            let total_ach = achievements.len() as i32;
                            let unlocked = achievements.iter().filter(|a| a.achieved == 1).count() as i32;
                            
                            // Also fetch achievement schema for names and icons
                            let schema_url = format!(
                                "{}?appid={}&key={}&format=json",
                                API_SCHEMA,
                                game.appid,
                                steam_key
                            );
                            
                            if let Ok(schema_response) = reqwest::blocking::get(&schema_url) {
                                if let Ok(schema_body) = schema_response.text() {
                                    if let Ok(schema_json) = serde_json::from_str::<serde_json::Value>(&schema_body) {
                                        if let Some(schema_arr) = schema_json["game"]["availableGameStats"]["achievements"].as_array() {
                                            let schema: Vec<AchievementSchema> = schema_arr
                                                .iter()
                                                .filter_map(|a| serde_json::from_value(a.clone()).ok())
                                                .collect();
                                            // Save detailed achievements to DB
                                            let _ = crate::db::save_game_achievements(&conn, &config.steam_id, game.appid, &schema, &achievements);
                                        }
                                    }
                                }
                            }
                            
                            let _ = crate::db::update_game_achievements(&conn, &config.steam_id, game.appid, &achievements);
                            let _ = progress_tx.send(ScrapeProgress::GameUpdated {
                                appid: game.appid,
                                unlocked,
                                total: total_ach,
                            });
                        } else {
                            // Game has no achievements
                            let _ = crate::db::mark_game_no_achievements(&conn, &config.steam_id, game.appid);
                            let _ = progress_tx.send(ScrapeProgress::GameUpdated {
                                appid: game.appid,
                                unlocked: 0,
                                total: 0,
                            });
                        }
                    }
                }
            }
            Err(_) => {
                // Skip this game on error, continue with others
            }
        }
        
        // Small delay to avoid rate limiting
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    // Reload all games with updated achievement data
    let games = crate::db::get_all_games(&conn, &config.steam_id)?;
    let _ = progress_tx.send(ScrapeProgress::Done { games });
    
    Ok(())
}

/// Fetch recently played games from Steam API (returns full game info)
pub fn fetch_recently_played_games(steam_key: &str, steam_id: u64, debug_output: bool) -> Result<Vec<SteamGame>, Box<dyn std::error::Error>> {
    let input = serde_json::json!({
        "steamid": steam_id,
        "count": 0  // 0 means return all recently played games
    });
    
    let url = format!(
        "{}?key={}&input_json={}&format=json",
        API_RECENTLY_PLAYED,
        steam_key,
        urlencoding::encode(&input.to_string())
    );
    
    let response = reqwest::blocking::get(&url)?;
    let body: serde_json::Value = response.json()?;
    
    // Debug output if enabled
    if debug_output {
        use std::io::Write;
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let mut debug_content = format!("=== Recently Played API Debug ===\n");
        debug_content.push_str(&format!("Timestamp: {}\n", timestamp));
        debug_content.push_str(&format!("Steam ID: {}\n", steam_id));
        debug_content.push_str(&format!("API URL: {}\n\n", API_RECENTLY_PLAYED));
        debug_content.push_str("=== Raw Response ===\n");
        debug_content.push_str(&serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string()));
        debug_content.push_str("\n\n=== Games List ===\n");
        
        if let Some(games) = body["response"]["games"].as_array() {
            debug_content.push_str(&format!("Total games in response: {}\n\n", games.len()));
            for (i, game) in games.iter().enumerate() {
                let appid = game["appid"].as_u64().unwrap_or(0);
                let name = game["name"].as_str().unwrap_or("Unknown");
                let playtime_2weeks = game["playtime_2weeks"].as_u64().unwrap_or(0);
                let playtime_forever = game["playtime_forever"].as_u64().unwrap_or(0);
                debug_content.push_str(&format!(
                    "{}. {} (appid: {}) - 2 weeks: {} min, total: {} min\n",
                    i + 1, name, appid, playtime_2weeks, playtime_forever
                ));
            }
        } else {
            debug_content.push_str("No games array found in response\n");
        }
        
        // Write to file
        if let Ok(mut file) = std::fs::File::create("recently_played_debug.txt") {
            let _ = file.write_all(debug_content.as_bytes());
        }
    }
    
    // Parse full game info (the API returns name, icon, playtime)
    let games: Vec<SteamGame> = body["response"]["games"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|g| serde_json::from_value(g.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    
    Ok(games)
}

/// Run the Update flow: fetch games, get recently played, scrape achievements for recent games
pub fn run_update_with_progress(progress_tx: Sender<UpdateProgress>) -> Result<(), Box<dyn std::error::Error>> {
    // Helper to log to ttb_log.txt
    fn update_log(msg: &str) {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("ttb_log.txt")
        {
            let _ = writeln!(file, "[{}] [update] {}", chrono::Local::now().format("%H:%M:%S"), msg);
        }
    }

    update_log("run_update_with_progress started");
    let config = Config::load();
    if !config.has_steam_credentials() {
        update_log("ERROR: No steam credentials");
        let _ = progress_tx.send(UpdateProgress::Error("Please configure steam_web_api_key and steam_id in config.toml".to_string()));
        return Ok(());
    }
    let steam_key = &config.steam_web_api_key;
    let steam_id = config.steam_id_u64().unwrap();
    update_log(&format!("Steam ID: {}", steam_id));

    // Step 1: Fetch owned games (quick)
    let _ = progress_tx.send(UpdateProgress::FetchingGames);
    update_log("Fetching owned games from Steam API...");

    let input = serde_json::json!({
        "steamid": steam_id,
        "include_appinfo": 1,
        "include_played_free_games": 1
    });

    let url = format!(
        "{}?key={}&input_json={}&format=json",
        API_OWNED_GAMES,
        steam_key,
        urlencoding::encode(&input.to_string())
    );

    update_log(&format!("Making HTTP request to: {}", &url[..url.find("key=").unwrap_or(0) + 10])); // Log URL without full key
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let response = match client.get(&url).send() {
        Ok(r) => r,
        Err(e) => {
            update_log(&format!("HTTP request failed: {}", e));
            let _ = progress_tx.send(UpdateProgress::Error(format!("Network error: {}", e)));
            return Ok(());
        }
    };
    update_log(&format!("Got response, status: {}", response.status()));

    // Check for API key errors
    if response.status() == reqwest::StatusCode::FORBIDDEN {
        update_log("ERROR: Steam API returned 403 Forbidden - invalid API key");
        let _ = progress_tx.send(UpdateProgress::Error("Invalid Steam API key. Please check your key at steamcommunity.com/dev/apikey".to_string()));
        return Ok(());
    }
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        update_log("ERROR: Steam API returned 401 Unauthorized - invalid API key");
        let _ = progress_tx.send(UpdateProgress::Error("Invalid Steam API key. Please check your key at steamcommunity.com/dev/apikey".to_string()));
        return Ok(());
    }
    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().unwrap_or_default();
        update_log(&format!("ERROR: Steam API returned {}: {}", status, body_text));
        // Check for "Access is denied" in body
        if body_text.contains("Access is denied") {
            let _ = progress_tx.send(UpdateProgress::Error("Invalid Steam API key. Please check your key at steamcommunity.com/dev/apikey".to_string()));
        } else {
            let _ = progress_tx.send(UpdateProgress::Error(format!("Steam API error: {} - {}", status, body_text)));
        }
        return Ok(());
    }

    let body: serde_json::Value = response.json()?;
    update_log("Parsed JSON response");

    let games: Vec<SteamGame> = body["response"]["games"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|g| serde_json::from_value(g.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    update_log(&format!("Parsed {} games", games.len()));

    update_log("Opening database connection...");
    let conn = crate::db::open_connection()?;
    update_log("Upserting games to database...");
    crate::db::upsert_games(&conn, &config.steam_id, &games)?;
    let total_games = games.len() as i32;
    let unplayed_games = games.iter().filter(|g| g.playtime_forever == 0).count() as i32;
    crate::db::insert_run_history(&conn, &config.steam_id, total_games, unplayed_games)?;
    
    // Step 2: Fetch recently played games
    update_log("Fetching recently played games...");
    let _ = progress_tx.send(UpdateProgress::FetchingRecentlyPlayed);
    
    let recent_games = fetch_recently_played_games(steam_key, steam_id, config.debug_recently_played)?;
    
    update_log(&format!("Got {} recently played games", recent_games.len()));
    if config.debug_recently_played || !recent_games.is_empty() {
        for game in &recent_games {
            update_log(&format!("Recent: {} (appid={}, playtime={}min)", 
                game.name, game.appid, game.playtime_forever));
        }
    }
    
    if recent_games.is_empty() {
        update_log("No recently played games");
        // No recently played games, we're done
        let games = crate::db::get_all_games(&conn, &config.steam_id)?;
        let _ = progress_tx.send(UpdateProgress::Done { games, updated_count: 0 });
        
        // Record the update time
        crate::db::record_last_update(&conn)?;
        return Ok(());
    }
    
    // Upsert recently played games (in case any are missing from owned games)
    update_log("Upserting recently played games to database...");
    crate::db::upsert_games(&conn, &config.steam_id, &recent_games)?;
    
    // Recalculate total games after adding recently played (some F2P games might not be in GetOwnedGames)
    let all_games_after_upsert = crate::db::get_all_games(&conn, &config.steam_id)?;
    let new_total = all_games_after_upsert.len() as i32;
    if new_total > total_games {
        // Update the run_history entry with the correct total
        crate::db::update_run_history_total(&conn, &config.steam_id, new_total)?;
    }
    
    // Get appids for filtering
    let recent_appids: Vec<u64> = recent_games.iter().map(|g| g.appid).collect();
    
    // Step 3: Scrape achievements for recently played games
    let games_to_scrape: Vec<Game> = crate::db::get_all_games(&conn, &config.steam_id)?
        .into_iter()
        .filter(|g| recent_appids.contains(&g.appid))
        .collect();
    
    let total = games_to_scrape.len() as i32;
    
    for (i, game) in games_to_scrape.iter().enumerate() {
        let _ = progress_tx.send(UpdateProgress::ScrapingAchievements {
            current: i as i32 + 1,
            total,
            game_name: game.name.clone(),
        });
        
        let url = format!(
            "{}?appid={}&key={}&steamid={}&format=json",
            API_ACHIEVEMENTS,
            game.appid,
            steam_key,
            steam_id
        );
        
        match reqwest::blocking::get(&url) {
            Ok(response) => {
                if let Ok(body) = response.text() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(achievements_arr) = json["playerstats"]["achievements"].as_array() {
                            let achievements: Vec<Achievement> = achievements_arr
                                .iter()
                                .filter_map(|a| serde_json::from_value(a.clone()).ok())
                                .collect();
                            let total_ach = achievements.len() as i32;
                            let unlocked = achievements.iter().filter(|a| a.achieved == 1).count() as i32;
                            
                            // Also fetch achievement schema for names and icons
                            let schema_url = format!(
                                "{}?appid={}&key={}&format=json",
                                API_SCHEMA,
                                game.appid,
                                steam_key
                            );
                            
                            if let Ok(schema_response) = reqwest::blocking::get(&schema_url) {
                                if let Ok(schema_body) = schema_response.text() {
                                    if let Ok(schema_json) = serde_json::from_str::<serde_json::Value>(&schema_body) {
                                        if let Some(schema_arr) = schema_json["game"]["availableGameStats"]["achievements"].as_array() {
                                            let schema: Vec<AchievementSchema> = schema_arr
                                                .iter()
                                                .filter_map(|a| serde_json::from_value(a.clone()).ok())
                                                .collect();
                                            // Save detailed achievements to DB
                                            let _ = crate::db::save_game_achievements(&conn, &config.steam_id, game.appid, &schema, &achievements);
                                        }
                                    }
                                }
                            }
                            
                            let _ = crate::db::update_game_achievements(&conn, &config.steam_id, game.appid, &achievements);
                            let _ = progress_tx.send(UpdateProgress::GameUpdated {
                                appid: game.appid,
                                unlocked,
                                total: total_ach,
                            });
                        } else {
                            // Game has no achievements
                            let _ = crate::db::mark_game_no_achievements(&conn, &config.steam_id, game.appid);
                            let _ = progress_tx.send(UpdateProgress::GameUpdated {
                                appid: game.appid,
                                unlocked: 0,
                                total: 0,
                            });
                        }
                    }
                }
            }
            Err(_) => {
                // Skip this game on error, continue with others
            }
        }
        
        // Small delay to avoid rate limiting
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    // Record the update time
    crate::db::record_last_update(&conn)?;
    
    // Reload all games with updated achievement data
    let games = crate::db::get_all_games(&conn, &config.steam_id)?;
    let _ = progress_tx.send(UpdateProgress::Done { games, updated_count: total });
    
    Ok(())
}

/// Refresh achievements for a single game
pub fn refresh_single_game(progress_tx: Sender<SingleGameRefreshProgress>, appid: u64) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load();
    if !config.has_steam_credentials() {
        let _ = progress_tx.send(SingleGameRefreshProgress::Error("Please configure steam_web_api_key and steam_id in config.toml".to_string()));
        return Ok(());
    }
    
    let steam_key = &config.steam_web_api_key;
    let steam_id = config.steam_id_u64().unwrap();
    let conn = crate::db::open_connection()?;
    
    let _ = progress_tx.send(SingleGameRefreshProgress::Refreshing { appid });
    
    // Fetch player achievements
    let url = format!(
        "{}?appid={}&key={}&steamid={}&format=json",
        API_ACHIEVEMENTS,
        appid,
        steam_key,
        steam_id
    );
    
    match reqwest::blocking::get(&url) {
        Ok(response) => {
            if let Ok(body) = response.text() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(achievements_arr) = json["playerstats"]["achievements"].as_array() {
                        let achievements: Vec<Achievement> = achievements_arr
                            .iter()
                            .filter_map(|a| serde_json::from_value(a.clone()).ok())
                            .collect();
                        
                        // Also fetch achievement schema for names and icons
                        let schema_url = format!(
                            "{}?appid={}&key={}&format=json",
                            API_SCHEMA,
                            appid,
                            steam_key
                        );
                        
                        if let Ok(schema_response) = reqwest::blocking::get(&schema_url) {
                            if let Ok(schema_body) = schema_response.text() {
                                if let Ok(schema_json) = serde_json::from_str::<serde_json::Value>(&schema_body) {
                                    if let Some(schema_arr) = schema_json["game"]["availableGameStats"]["achievements"].as_array() {
                                        let schema: Vec<AchievementSchema> = schema_arr
                                            .iter()
                                            .filter_map(|a| serde_json::from_value(a.clone()).ok())
                                            .collect();
                                        // Save detailed achievements to DB
                                        let _ = crate::db::save_game_achievements(&conn, &config.steam_id, appid, &schema, &achievements);
                                    }
                                }
                            }
                        }
                        
                        let _ = crate::db::update_game_achievements(&conn, &config.steam_id, appid, &achievements);
                        
                        // Reload the game and its achievements
                        let games = crate::db::get_all_games(&conn, &config.steam_id)?;
                        if let Some(game) = games.into_iter().find(|g| g.appid == appid) {
                            let game_achievements = crate::db::get_game_achievements(&conn, &config.steam_id, appid)?;
                            let _ = progress_tx.send(SingleGameRefreshProgress::Done { 
                                appid, 
                                game,
                                achievements: game_achievements,
                            });
                        } else {
                            let _ = progress_tx.send(SingleGameRefreshProgress::Error("Game not found after refresh".to_string()));
                        }
                    } else {
                        // Game has no achievements
                        let _ = crate::db::mark_game_no_achievements(&conn, &config.steam_id, appid);
                        let games = crate::db::get_all_games(&conn, &config.steam_id)?;
                        if let Some(game) = games.into_iter().find(|g| g.appid == appid) {
                            let _ = progress_tx.send(SingleGameRefreshProgress::Done { 
                                appid, 
                                game,
                                achievements: vec![],
                            });
                        } else {
                            let _ = progress_tx.send(SingleGameRefreshProgress::Error("Game not found after refresh".to_string()));
                        }
                    }
                } else {
                    let _ = progress_tx.send(SingleGameRefreshProgress::Error("Failed to parse achievements response".to_string()));
                }
            } else {
                let _ = progress_tx.send(SingleGameRefreshProgress::Error("Failed to read achievements response".to_string()));
            }
        }
        Err(e) => {
            let _ = progress_tx.send(SingleGameRefreshProgress::Error(format!("Failed to fetch achievements: {}", e)));
        }
    }
    
    Ok(())
}
