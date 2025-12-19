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
    let conn = crate::db::open_connection()?;
    crate::db::upsert_games(&conn, &config.steam_id, &games)?;
    crate::db::insert_run_history(&conn, &config.steam_id, total)?;
    
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
    crate::db::insert_run_history(&conn, &config.steam_id, total_games)?;
    
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

/// Fetch recently played games from Steam API (returns appids)
pub fn fetch_recently_played_games(steam_key: &str, steam_id: u64) -> Result<Vec<u64>, Box<dyn std::error::Error>> {
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
    
    let appids: Vec<u64> = body["response"]["games"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|g| g["appid"].as_u64())
                .collect()
        })
        .unwrap_or_default();
    
    Ok(appids)
}

/// Run the Update flow: fetch games, get recently played, scrape achievements for recent games
pub fn run_update_with_progress(progress_tx: Sender<UpdateProgress>) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load();
    if !config.has_steam_credentials() {
        let _ = progress_tx.send(UpdateProgress::Error("Please configure steam_web_api_key and steam_id in config.toml".to_string()));
        return Ok(());
    }
    let steam_key = &config.steam_web_api_key;
    let steam_id = config.steam_id_u64().unwrap();
    
    // Step 1: Fetch owned games (quick)
    let _ = progress_tx.send(UpdateProgress::FetchingGames);
    
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
    crate::db::insert_run_history(&conn, &config.steam_id, total_games)?;
    
    // Step 2: Fetch recently played games
    let _ = progress_tx.send(UpdateProgress::FetchingRecentlyPlayed);
    
    let recent_appids = fetch_recently_played_games(steam_key, steam_id)?;
    
    if recent_appids.is_empty() {
        // No recently played games, we're done
        let games = crate::db::get_all_games(&conn, &config.steam_id)?;
        let _ = progress_tx.send(UpdateProgress::Done { games, updated_count: 0 });
        
        // Record the update time
        crate::db::record_last_update(&conn)?;
        return Ok(());
    }
    
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
