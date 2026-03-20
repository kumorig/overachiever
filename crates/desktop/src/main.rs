// Hide console window on Windows in release builds (but not for CLI modes)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cjk_font;
mod cloud_sync;
mod config;
mod db;
mod fonts;
mod icon_cache;
mod steam_api;
mod steam_library;
mod steam_config;
mod steamspy;
mod ttb;
mod ui;

use app::SteamOverachieverApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--update") {
        // Headless update mode: run update, save stats, exit
        std::process::exit(run_headless_update());
    }

    if args.iter().any(|a| a == "--schedule") {
        // Register/update a daily Windows Task Scheduler task
        std::process::exit(setup_schedule());
    }

    if args.iter().any(|a| a == "--unschedule") {
        // Remove the daily Windows Task Scheduler task
        std::process::exit(remove_schedule());
    }

    run_gui()
}

/// Headless update: run the same update logic as the GUI but without a window
fn run_headless_update() -> i32 {
    attach_console();

    let cfg = config::Config::load();
    if !cfg.has_steam_credentials() {
        eprintln!("Error: Steam credentials not configured. Run the app normally first.");
        return 1;
    }

    println!("Overachiever: starting headless update...");

    let (tx, rx) = std::sync::mpsc::channel();
    if let Err(e) = steam_api::run_update_with_progress(tx) {
        eprintln!("Update failed: {}", e);
        return 1;
    }

    // Drain the channel to find the final result
    let mut updated_count = 0i32;
    let mut games = Vec::new();
    let mut had_error = false;
    while let Ok(msg) = rx.try_recv() {
        match msg {
            steam_api::UpdateProgress::Done { games: g, updated_count: c } => {
                games = g;
                updated_count = c;
            }
            steam_api::UpdateProgress::Error(e) => {
                eprintln!("Update error: {}", e);
                had_error = true;
            }
            _ => {}
        }
    }

    if had_error {
        return 1;
    }

    // Save achievement history (same as GUI does after update)
    if let Ok(conn) = db::open_connection() {
        let games_with_ach: Vec<_> = games.iter()
            .filter(|g| g.achievements_total.map(|t| t > 0).unwrap_or(false))
            .collect();

        if !games_with_ach.is_empty() {
            let total: i32 = games_with_ach.iter().filter_map(|g| g.achievements_total).sum();
            let unlocked: i32 = games_with_ach.iter().filter_map(|g| g.achievements_unlocked).sum();
            let unplayed = games_with_ach.iter().filter(|g| g.playtime_forever == 0).count() as i32;

            let pcts: Vec<f32> = games_with_ach.iter()
                .filter(|g| g.playtime_forever > 0)
                .filter_map(|g| g.completion_percent())
                .collect();
            let avg = if pcts.is_empty() { 0.0 } else { pcts.iter().sum::<f32>() / pcts.len() as f32 };

            let _ = db::update_latest_run_history_unplayed(&conn, &cfg.steam_id, unplayed);
            let _ = db::backfill_run_history_unplayed(&conn, &cfg.steam_id, unplayed);
            let _ = db::insert_achievement_history(&conn, &cfg.steam_id, total, unlocked, games_with_ach.len() as i32, avg);
        }
    }

    println!("Update complete. {} games updated.", updated_count);
    0
}

const TASK_NAME: &str = "OverachieverDailyUpdate";

/// Create or update a daily task in Windows Task Scheduler
fn setup_schedule() -> i32 {
    attach_console();

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to get executable path: {}", e);
            return 1;
        }
    };

    let exe_str = exe.to_string_lossy();

    // /F overwrites any existing task with the same name
    let status = std::process::Command::new("schtasks")
        .args([
            "/Create",
            "/F",
            "/SC", "DAILY",
            "/TN", TASK_NAME,
            "/TR", &format!("\"{}\" --update", exe_str),
            "/ST", "20:00",
        ])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("Scheduled daily update task \"{}\" at 20:00.", TASK_NAME);
            println!("  Executable: {} --update", exe_str);
            println!("  To remove:  overachiever --unschedule");
            0
        }
        Ok(s) => {
            eprintln!("schtasks exited with code {}. Try running as administrator.", s.code().unwrap_or(-1));
            1
        }
        Err(e) => {
            eprintln!("Failed to run schtasks: {}", e);
            1
        }
    }
}

/// Remove the daily task from Windows Task Scheduler
fn remove_schedule() -> i32 {
    attach_console();

    let status = std::process::Command::new("schtasks")
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("Removed scheduled task \"{}\".", TASK_NAME);
            0
        }
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            if code == 1 {
                println!("No scheduled task \"{}\" found.", TASK_NAME);
            } else {
                eprintln!("schtasks exited with code {}.", code);
            }
            code
        }
        Err(e) => {
            eprintln!("Failed to run schtasks: {}", e);
            1
        }
    }
}

fn attach_console() {
    #[cfg(windows)]
    unsafe { windows_sys::Win32::System::Console::AttachConsole(u32::MAX); }
}

fn run_gui() -> eframe::Result<()> {
    // Load icon for window
    let icon_data = include_bytes!("../../../assets/icon.png");
    let icon_image = image::load_from_memory(icon_data).expect("Failed to load icon");
    let icon_rgba = icon_image.to_rgba8();
    let (width, height) = icon_rgba.dimensions();
    let icon = egui::IconData {
        rgba: icon_rgba.into_raw(),
        width,
        height,
    };

    // Load config to get saved window state
    let config = config::Config::load();

    // Build viewport with saved or default size/position
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([
            config.window_width.unwrap_or(1024.0),
            config.window_height.unwrap_or(768.0),
        ])
        .with_icon(icon);

    // Apply saved position if available
    if let (Some(x), Some(y)) = (config.window_x, config.window_y) {
        viewport = viewport.with_position([x, y]);
    }

    // Apply maximized state
    if config.window_maximized {
        viewport = viewport.with_maximized(true);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Overachiever v3",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            // Load config and apply font settings
            let config = config::Config::load();
            app::panels::top::fonts::apply_font_settings(&cc.egui_ctx, &config);

            Ok(Box::new(SteamOverachieverApp::new()))
        }),
    )
}
