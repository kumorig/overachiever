//! Standalone CLI that backfills the local DB from the cloud export on
//! overachiever.space after an accidental local data loss.
//!
//! Three things are merged, all additive (never overwrites or deletes):
//!
//!   1. run_history rows older than the local earliest run_at
//!   2. achievement_history rows older than the local earliest recorded_at
//!   3. games (+ their achievements) that exist in the remote export but not
//!      in the local DB — typically private games that GetOwnedGames refuses
//!      to return after a fresh scan.
//!
//! The backfill in (3) restores continuity to the four graphs (Total games,
//! Unplayed games, Avg game completion, Overall Achievements) which would
//! otherwise drop at the gap boundary because ~N private games are missing
//! from the rebuilt library. Inserted games have `last_achievement_scrape`
//! left NULL so the next Update in the app repopulates their icons/names.
//!
//! Usage (from repo root):
//!   cargo run -p overachiever-desktop --bin merge-cloud-history
//!   cargo run -p overachiever-desktop --bin merge-cloud-history -- --apply
//!
//! `--release` or `--debug` makes no difference to the data paths — both
//! read/write `%APPDATA%\Overachiever\data\steam_overachiever.db`.

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_SERVER_URL: &str = "https://overachiever.space";

#[derive(Debug, Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    steam_id: String,
    #[serde(default)]
    cloud_token: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct RunHistoryRow {
    #[serde(default)]
    run_at: Option<DateTime<Utc>>,
    #[serde(default)]
    total_games: i32,
    #[serde(default)]
    unplayed_games: i32,
    #[serde(default)]
    unplayed_games_total: i32,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct AchHistoryRow {
    #[serde(default)]
    recorded_at: Option<DateTime<Utc>>,
    #[serde(default)]
    total_achievements: i32,
    #[serde(default)]
    unlocked_achievements: i32,
    #[serde(default)]
    games_with_achievements: i32,
    #[serde(default)]
    avg_completion_percent: f32,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct RemoteGame {
    #[serde(default)]
    appid: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    playtime_forever: u32,
    #[serde(default)]
    rtime_last_played: Option<u32>,
    #[serde(default)]
    img_icon_url: Option<String>,
    #[serde(default)]
    added_at: Option<DateTime<Utc>>,
    #[serde(default)]
    achievements_total: Option<i32>,
    #[serde(default)]
    achievements_unlocked: Option<i32>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct RemoteAchievement {
    #[serde(default)]
    appid: u64,
    #[serde(default)]
    apiname: String,
    #[serde(default)]
    achieved: bool,
    #[serde(default)]
    unlocktime: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Default)]
struct CloudSyncBundle {
    #[serde(default)]
    games: Vec<RemoteGame>,
    #[serde(default)]
    achievements: Vec<RemoteAchievement>,
    #[serde(default)]
    run_history: Vec<RunHistoryRow>,
    #[serde(default)]
    achievement_history: Vec<AchHistoryRow>,
}

fn app_data_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "Overachiever")
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let apply = std::env::args().any(|a| a == "--apply");
    let trim_incomplete = std::env::args().any(|a| a == "--trim-incomplete");

    let data_dir = app_data_dir();
    let config_path = data_dir.join("config.toml");
    let db_path = data_dir.join("steam_overachiever.db");

    println!("Data dir: {}", data_dir.display());
    println!("Config:   {}", config_path.display());
    println!("DB:       {}", db_path.display());
    println!();

    if !db_path.exists() {
        return Err(format!("Local DB not found at {}", db_path.display()).into());
    }

    let config_str = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Cannot read config.toml: {}", e))?;
    let config: ConfigFile = toml::from_str(&config_str)
        .map_err(|e| format!("Cannot parse config.toml: {}", e))?;

    let token = config
        .cloud_token
        .as_deref()
        .ok_or("No cloud_token in config.toml — run the app and link to cloud first")?;
    if config.steam_id.is_empty() {
        return Err("No steam_id in config.toml".into());
    }
    let steam_id = config.steam_id.as_str();
    println!("Steam ID: {}", steam_id);

    println!("Downloading from {} ...", DEFAULT_SERVER_URL);
    let url = format!("{}/api/sync/download", DEFAULT_SERVER_URL);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body).into());
    }
    let body = resp.text()?;
    let remote: CloudSyncBundle = serde_json::from_str(&body)
        .map_err(|e| format!("Cannot parse remote JSON: {}", e))?;
    println!(
        "Remote: {} games, {} achievements, {} run_history, {} ach_history",
        remote.games.len(),
        remote.achievements.len(),
        remote.run_history.len(),
        remote.achievement_history.len()
    );

    let conn = Connection::open(&db_path)?;
    let (local_run, local_ach) = read_local_history(&conn, steam_id)?;
    let local_appids = read_local_appids(&conn, steam_id)?;
    let local_private_count = count_local_private(&conn, steam_id)?;
    drop(conn);
    println!(
        "Local:  {} games ({} flagged private), {} run_history, {} ach_history",
        local_appids.len(),
        local_private_count,
        local_run.len(),
        local_ach.len()
    );
    println!();

    summarize(
        "run_history (local) ",
        &collect_ts(&local_run, |r| r.run_at),
    );
    summarize(
        "run_history (remote)",
        &collect_ts(&remote.run_history, |r| r.run_at),
    );
    summarize(
        "ach_history (local) ",
        &collect_ts(&local_ach, |r| r.recorded_at),
    );
    summarize(
        "ach_history (remote)",
        &collect_ts(&remote.achievement_history, |r| r.recorded_at),
    );
    println!();

    // ------- history merge plan (rows older than local earliest) -------
    let run_cutoff = collect_ts(&local_run, |r| r.run_at).into_iter().min();
    let ach_cutoff = collect_ts(&local_ach, |r| r.recorded_at).into_iter().min();

    let run_to_insert: Vec<RunHistoryRow> = remote
        .run_history
        .iter()
        .filter(|r| match (r.run_at, run_cutoff) {
            (Some(t), Some(c)) => t < c,
            (Some(_), None) => true,
            _ => false,
        })
        .cloned()
        .collect();
    let ach_to_insert: Vec<AchHistoryRow> = remote
        .achievement_history
        .iter()
        .filter(|r| match (r.recorded_at, ach_cutoff) {
            (Some(t), Some(c)) => t < c,
            (Some(_), None) => true,
            _ => false,
        })
        .cloned()
        .collect();

    // ------- games backfill plan (in remote but missing locally) -------
    let games_to_insert: Vec<RemoteGame> = remote
        .games
        .iter()
        .filter(|g| g.appid != 0 && !local_appids.contains(&g.appid))
        .cloned()
        .collect();
    let missing_appids: HashSet<u64> = games_to_insert.iter().map(|g| g.appid).collect();
    let achievements_to_insert: Vec<RemoteAchievement> = remote
        .achievements
        .iter()
        .filter(|a| a.appid != 0 && !a.apiname.is_empty() && missing_appids.contains(&a.appid))
        .cloned()
        .collect();

    let missing_with_ach = games_to_insert
        .iter()
        .filter(|g| g.achievements_total.unwrap_or(0) > 0)
        .count();
    let missing_total_ach: i32 = games_to_insert
        .iter()
        .filter_map(|g| g.achievements_total)
        .sum();
    let missing_unlocked_ach: i32 = games_to_insert
        .iter()
        .filter_map(|g| g.achievements_unlocked)
        .sum();

    println!("Merge plan:");
    if let Some(c) = run_cutoff {
        println!("  run_history cutoff:            < {}", fmt(c));
    } else {
        println!("  run_history cutoff:            (local empty — would insert all)");
    }
    if let Some(c) = ach_cutoff {
        println!("  achievement_history cutoff:    < {}", fmt(c));
    } else {
        println!("  achievement_history cutoff:    (local empty — would insert all)");
    }
    println!("  run_history:                   +{} rows", run_to_insert.len());
    println!("  achievement_history:           +{} rows", ach_to_insert.len());
    println!("  games (remote \\ local):        +{} rows", games_to_insert.len());
    println!(
        "      of which with achievements: {} games, +{} total / +{} unlocked achievements",
        missing_with_ach, missing_total_ach, missing_unlocked_ach
    );
    println!("  achievements (for those games): +{} rows", achievements_to_insert.len());

    // ------- trim plan: post-gap local rows captured while scrape was incomplete -------
    // The max games_with_achievements seen in remote (pre-wipe) rows is the
    // "caught up" threshold. Any local row post-gap with a lower value was
    // recorded mid-scrape and causes the spurious dip in the graph.
    let remote_max_games_w_ach = remote
        .achievement_history
        .iter()
        .map(|r| r.games_with_achievements)
        .max()
        .unwrap_or(0);
    let ach_trim_candidates: Vec<&AchHistoryRow> = local_ach
        .iter()
        .filter(|r| r.games_with_achievements < remote_max_games_w_ach)
        .collect();
    if trim_incomplete {
        println!(
            "  trim ach_history (games_w_ach < {}): −{} rows",
            remote_max_games_w_ach,
            ach_trim_candidates.len()
        );
    } else {
        println!(
            "  (pass --trim-incomplete to also remove {} local ach_history rows with games_w_ach < {})",
            ach_trim_candidates.len(),
            remote_max_games_w_ach
        );
    }
    println!();

    print_run_samples(
        "run_history to insert (last 5 before gap)",
        &last_n(&run_to_insert, 5),
    );
    print_run_samples(
        "run_history local (first 3 after gap)",
        &first_n(&local_run, 3),
    );
    print_ach_samples(
        "achievement_history to insert (last 5 before gap)",
        &last_n(&ach_to_insert, 5),
    );
    print_ach_samples(
        "achievement_history local (first 3 after gap)",
        &first_n(&local_ach, 3),
    );
    print_missing_games_sample("games to backfill (sample of up to 20 with achievements)", &games_to_insert, 20);

    if !apply {
        println!();
        println!("Dry-run complete. Re-run with --apply to write changes.");
        return Ok(());
    }

    let trim_count = if trim_incomplete { ach_trim_candidates.len() } else { 0 };
    let anything_to_do = !run_to_insert.is_empty()
        || !ach_to_insert.is_empty()
        || !games_to_insert.is_empty()
        || !achievements_to_insert.is_empty()
        || trim_count > 0;
    if !anything_to_do {
        println!();
        println!("Nothing to insert — local already covers the remote data.");
        return Ok(());
    }

    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let backup = data_dir.join(format!("steam_overachiever.db.bak-{}", ts));
    std::fs::copy(&db_path, &backup)?;
    println!();
    println!("Backup created: {}", backup.display());

    let mut conn = Connection::open(&db_path)?;
    let tx = conn.transaction()?;

    // History
    for r in &run_to_insert {
        let run_at = r.run_at.ok_or("remote run_history row missing run_at")?;
        tx.execute(
            "INSERT INTO run_history (steam_id, run_at, total_games, unplayed_games, unplayed_games_total)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                steam_id,
                run_at.to_rfc3339(),
                r.total_games,
                r.unplayed_games,
                r.unplayed_games_total,
            ],
        )?;
    }
    for r in &ach_to_insert {
        let recorded_at = r.recorded_at.ok_or("remote achievement_history row missing recorded_at")?;
        tx.execute(
            "INSERT INTO achievement_history (steam_id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                steam_id,
                recorded_at.to_rfc3339(),
                r.total_achievements,
                r.unlocked_achievements,
                r.games_with_achievements,
                r.avg_completion_percent,
            ],
        )?;
    }

    // Games (only those missing locally). last_achievement_scrape left NULL
    // so the next Update in the GUI repopulates names/icons/metadata.
    for g in &games_to_insert {
        let added_at = g.added_at.unwrap_or_else(Utc::now).to_rfc3339();
        tx.execute(
            "INSERT INTO games (steam_id, appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at, achievements_total, achievements_unlocked)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                steam_id,
                g.appid as i64,
                g.name,
                g.playtime_forever,
                g.rtime_last_played,
                g.img_icon_url,
                added_at,
                g.achievements_total,
                g.achievements_unlocked,
            ],
        )?;
    }

    // Achievements for those games. Blank metadata — gets filled by next scrape.
    for a in &achievements_to_insert {
        tx.execute(
            "INSERT OR IGNORE INTO achievements (steam_id, appid, apiname, name, description, icon, icon_gray, achieved, unlocktime)
             VALUES (?1, ?2, ?3, '', NULL, '', '', ?4, ?5)",
            rusqlite::params![
                steam_id,
                a.appid as i64,
                a.apiname,
                if a.achieved { 1 } else { 0 },
                a.unlocktime.map(|t| t.timestamp()),
            ],
        )?;
    }

    // Trim achievement_history rows captured mid-scrape (games_w_ach below
    // the pre-wipe peak). These are the ones that cause the spurious drop in
    // the graph right at the gap boundary.
    if trim_incomplete && remote_max_games_w_ach > 0 {
        tx.execute(
            "DELETE FROM achievement_history
             WHERE steam_id = ?1 AND games_with_achievements < ?2",
            rusqlite::params![steam_id, remote_max_games_w_ach],
        )?;
    }

    tx.commit()?;

    println!(
        "Applied: +{} run_history, +{} achievement_history, +{} games, +{} achievements, −{} trimmed ach_history",
        run_to_insert.len(),
        ach_to_insert.len(),
        games_to_insert.len(),
        achievements_to_insert.len(),
        trim_count
    );
    println!("If anything looks wrong, restore from: {}", backup.display());
    println!();
    println!("Next: run an Update in the app — it'll repopulate icons/names for the backfilled games.");

    Ok(())
}

fn read_local_history(
    conn: &Connection,
    steam_id: &str,
) -> rusqlite::Result<(Vec<RunHistoryRow>, Vec<AchHistoryRow>)> {
    let mut run = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT run_at, total_games, COALESCE(unplayed_games, 0), COALESCE(unplayed_games_total, 0)
             FROM run_history WHERE steam_id = ?1 ORDER BY run_at",
        )?;
        let iter = stmt.query_map([steam_id], |row| {
            let run_at: String = row.get(0)?;
            Ok(RunHistoryRow {
                run_at: DateTime::parse_from_rfc3339(&run_at)
                    .ok()
                    .map(|t| t.with_timezone(&Utc)),
                total_games: row.get(1)?,
                unplayed_games: row.get(2)?,
                unplayed_games_total: row.get(3)?,
            })
        })?;
        for r in iter {
            run.push(r?);
        }
    }

    let mut ach = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent
             FROM achievement_history WHERE steam_id = ?1 ORDER BY recorded_at",
        )?;
        let iter = stmt.query_map([steam_id], |row| {
            let recorded_at: String = row.get(0)?;
            Ok(AchHistoryRow {
                recorded_at: DateTime::parse_from_rfc3339(&recorded_at)
                    .ok()
                    .map(|t| t.with_timezone(&Utc)),
                total_achievements: row.get(1)?,
                unlocked_achievements: row.get(2)?,
                games_with_achievements: row.get(3)?,
                avg_completion_percent: row.get(4)?,
            })
        })?;
        for r in iter {
            ach.push(r?);
        }
    }
    Ok((run, ach))
}

fn read_local_appids(conn: &Connection, steam_id: &str) -> rusqlite::Result<HashSet<u64>> {
    let mut stmt = conn.prepare("SELECT appid FROM games WHERE steam_id = ?1")?;
    let iter = stmt.query_map([steam_id], |row| row.get::<_, i64>(0))?;
    let mut set = HashSet::new();
    for a in iter {
        set.insert(a? as u64);
    }
    Ok(set)
}

fn count_local_private(conn: &Connection, steam_id: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM games WHERE steam_id = ?1 AND steam_private = 1",
        [steam_id],
        |row| row.get(0),
    )
}

fn collect_ts<T, F>(rows: &[T], f: F) -> Vec<DateTime<Utc>>
where
    F: Fn(&T) -> Option<DateTime<Utc>>,
{
    rows.iter().filter_map(|r| f(r)).collect()
}

fn summarize(label: &str, ts: &[DateTime<Utc>]) {
    match (ts.iter().min(), ts.iter().max()) {
        (Some(lo), Some(hi)) => println!(
            "  {}: {:>5} rows, {} ..= {}",
            label,
            ts.len(),
            fmt(*lo),
            fmt(*hi)
        ),
        _ => println!("  {}: 0 rows", label),
    }
}

fn fmt(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M").to_string()
}

fn first_n<T: Clone>(v: &[T], n: usize) -> Vec<T> {
    v.iter().take(n).cloned().collect()
}

fn last_n<T: Clone>(v: &[T], n: usize) -> Vec<T> {
    let start = v.len().saturating_sub(n);
    v[start..].to_vec()
}

fn print_run_samples(label: &str, rows: &[RunHistoryRow]) {
    if rows.is_empty() {
        return;
    }
    println!("{}:", label);
    for r in rows {
        let ts = r.run_at.map(fmt).unwrap_or_else(|| "?".to_string());
        println!(
            "  {}  total={:>5}  unplayed={:>5}  unplayed_total={:>5}",
            ts, r.total_games, r.unplayed_games, r.unplayed_games_total
        );
    }
}

fn print_ach_samples(label: &str, rows: &[AchHistoryRow]) {
    if rows.is_empty() {
        return;
    }
    println!("{}:", label);
    for r in rows {
        let ts = r.recorded_at.map(fmt).unwrap_or_else(|| "?".to_string());
        println!(
            "  {}  total={:>6}  unlocked={:>6}  games_w_ach={:>5}  avg_pct={:.2}",
            ts,
            r.total_achievements,
            r.unlocked_achievements,
            r.games_with_achievements,
            r.avg_completion_percent
        );
    }
}

fn print_missing_games_sample(label: &str, games: &[RemoteGame], limit: usize) {
    if games.is_empty() {
        return;
    }
    println!("{}:", label);
    let mut with_ach: Vec<&RemoteGame> = games
        .iter()
        .filter(|g| g.achievements_total.unwrap_or(0) > 0)
        .collect();
    // Sort by unlocked desc — most interesting first
    with_ach.sort_by(|a, b| {
        b.achievements_unlocked
            .unwrap_or(0)
            .cmp(&a.achievements_unlocked.unwrap_or(0))
    });
    for g in with_ach.iter().take(limit) {
        println!(
            "  appid={:>10}  {:>3}/{:<3}  playtime={:>6}min  {}",
            g.appid,
            g.achievements_unlocked.unwrap_or(0),
            g.achievements_total.unwrap_or(0),
            g.playtime_forever,
            truncate(&g.name, 60),
        );
    }
    if with_ach.len() > limit {
        println!("  ... and {} more", with_ach.len() - limit);
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
