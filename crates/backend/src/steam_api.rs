//! Steam API calls from the backend

use overachiever_core::{SteamGame, Achievement, AchievementSchema};

const API_OWNED_GAMES: &str = "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";
const API_RECENTLY_PLAYED: &str = "https://api.steampowered.com/IPlayerService/GetRecentlyPlayedGames/v1/";
const API_ACHIEVEMENTS: &str = "http://api.steampowered.com/ISteamUserStats/GetPlayerAchievements/v0001/";
const API_SCHEMA: &str = "http://api.steampowered.com/ISteamUserStats/GetSchemaForGame/v2/";

pub async fn fetch_owned_games(
    steam_key: &str,
    steam_id: u64,
) -> Result<Vec<SteamGame>, Box<dyn std::error::Error + Send + Sync>> {
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
    
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;
    let body: serde_json::Value = response.json().await?;
    
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

pub async fn fetch_recently_played(
    steam_key: &str,
    steam_id: u64,
) -> Result<Vec<u64>, Box<dyn std::error::Error + Send + Sync>> {
    let input = serde_json::json!({
        "steamid": steam_id,
        "count": 0
    });
    
    let url = format!(
        "{}?key={}&input_json={}&format=json",
        API_RECENTLY_PLAYED,
        steam_key,
        urlencoding::encode(&input.to_string())
    );
    
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;
    let body: serde_json::Value = response.json().await?;
    
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

pub async fn fetch_achievements(
    steam_key: &str,
    steam_id: u64,
    appid: u64,
) -> Result<Vec<Achievement>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "{}?appid={}&key={}&steamid={}&format=json",
        API_ACHIEVEMENTS, appid, steam_key, steam_id
    );
    
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;
    let body: serde_json::Value = response.json().await?;
    
    let achievements: Vec<Achievement> = body["playerstats"]["achievements"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| serde_json::from_value(a.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    
    Ok(achievements)
}

pub async fn fetch_achievement_schema(
    steam_key: &str,
    appid: u64,
) -> Result<Vec<AchievementSchema>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "{}?appid={}&key={}&format=json",
        API_SCHEMA, appid, steam_key
    );
    
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;
    let body: serde_json::Value = response.json().await?;
    
    let schema: Vec<AchievementSchema> = body["game"]["availableGameStats"]["achievements"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| serde_json::from_value(a.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    
    Ok(schema)
}
