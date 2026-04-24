//! Standalone CLI that bundles / unbundles the entire local Overachiever
//! profile (DB + config, optionally icon cache) into a single zip so you
//! can move it between machines via a USB stick, Google Drive, etc.
//!
//! Unlike cloud sync, this is a byte-faithful copy — no filtering, no
//! lossy repacking. Private games, `first_plays`, my_ttb_* reports,
//! app_settings — all of it.
//!
//! Usage:
//!   cargo run -p overachiever-desktop --bin profile-bundle -- export <out.zip>
//!   cargo run -p overachiever-desktop --bin profile-bundle -- export <out.zip> --include-icons
//!   cargo run -p overachiever-desktop --bin profile-bundle -- import <in.zip>
//!
//! Always close the app before running (SQLite file lock, and import
//! overwrites the live DB). The zip contains your Steam API key and
//! cloud token — treat it like a password.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

fn app_data_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "Overachiever")
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    exported_at: DateTime<Utc>,
    hostname: Option<String>,
    db_size: u64,
    has_config: bool,
    includes_icons: bool,
    icon_count: usize,
}

fn hostname() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let include_icons = args.iter().any(|a| a == "--include-icons");
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();

    let mode = positional.first().map(|s| s.as_str()).unwrap_or("");
    let path_arg = positional.get(1).cloned().cloned();

    match mode {
        "export" => {
            let path = path_arg.ok_or_else(|| {
                "usage: profile-bundle export <file.zip> [--include-icons]".to_string()
            })?;
            export_bundle(Path::new(&path), include_icons)?;
        }
        "import" => {
            let path = path_arg
                .ok_or_else(|| "usage: profile-bundle import <file.zip>".to_string())?;
            import_bundle(Path::new(&path))?;
        }
        _ => {
            eprintln!("usage:");
            eprintln!("  profile-bundle export <file.zip> [--include-icons]");
            eprintln!("  profile-bundle import <file.zip>");
            std::process::exit(1);
        }
    }
    Ok(())
}

fn export_bundle(out: &Path, include_icons: bool) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app_data_dir();
    let db_path = data_dir.join("steam_overachiever.db");
    let cfg_path = data_dir.join("config.toml");
    let icon_dir = data_dir.join("icon_cache");

    if !db_path.exists() {
        return Err(format!("No DB at {}", db_path.display()).into());
    }

    println!("Data dir: {}", data_dir.display());
    println!("Writing:  {}", out.display());
    println!();

    let file = File::create(out)?;
    let mut zip = ZipWriter::new(file);
    let opts: SimpleFileOptions = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .large_file(true);

    // DB
    let db_bytes = fs::read(&db_path)?;
    let db_size = db_bytes.len() as u64;
    zip.start_file("steam_overachiever.db", opts)?;
    zip.write_all(&db_bytes)?;
    println!(
        "  + steam_overachiever.db ({:.2} MB)",
        db_size as f64 / 1_048_576.0
    );
    drop(db_bytes);

    // config.toml (contains cloud_token + steam_web_api_key — sensitive)
    let has_config = cfg_path.exists();
    if has_config {
        let bytes = fs::read(&cfg_path)?;
        zip.start_file("config.toml", opts)?;
        zip.write_all(&bytes)?;
        println!(
            "  + config.toml ({} bytes) — contains API key + cloud token",
            bytes.len()
        );
    } else {
        println!("  (no config.toml found — skipping)");
    }

    // icon_cache (optional, flat directory)
    let mut icon_count = 0usize;
    if include_icons && icon_dir.is_dir() {
        for entry in fs::read_dir(&icon_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name();
            let zip_path = format!("icon_cache/{}", name.to_string_lossy());
            let bytes = fs::read(entry.path())?;
            zip.start_file(&zip_path, opts)?;
            zip.write_all(&bytes)?;
            icon_count += 1;
        }
        println!("  + icon_cache/ ({} files)", icon_count);
    } else if include_icons {
        println!("  (icon_cache dir not found — skipping)");
    }

    // Manifest
    let manifest = Manifest {
        exported_at: Utc::now(),
        hostname: hostname(),
        db_size,
        has_config,
        includes_icons: icon_count > 0,
        icon_count,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    zip.start_file("manifest.json", opts)?;
    zip.write_all(manifest_json.as_bytes())?;

    zip.finish()?;

    let final_size = fs::metadata(out)?.len();
    println!();
    println!(
        "Done. Bundle: {} ({:.2} MB)",
        out.display(),
        final_size as f64 / 1_048_576.0
    );
    println!();
    println!("SENSITIVE: this zip includes your Steam API key and cloud JWT.");
    println!("Treat it like a password. Don't post it in public shares.");
    Ok(())
}

fn import_bundle(src: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !src.exists() {
        return Err(format!("Bundle not found: {}", src.display()).into());
    }
    let data_dir = app_data_dir();
    fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("steam_overachiever.db");
    let cfg_path = data_dir.join("config.toml");
    let icon_dir = data_dir.join("icon_cache");

    println!("Data dir: {}", data_dir.display());
    println!("Reading:  {}", src.display());
    println!();

    let mut zip = ZipArchive::new(File::open(src)?)?;

    // Peek at manifest, if present, for friendlier output
    if let Ok(mut m) = zip.by_name("manifest.json") {
        let mut s = String::new();
        m.read_to_string(&mut s)?;
        if let Ok(manifest) = serde_json::from_str::<Manifest>(&s) {
            println!(
                "Bundle manifest: exported {} on {}",
                manifest.exported_at.format("%Y-%m-%d %H:%M UTC"),
                manifest.hostname.unwrap_or_else(|| "<unknown host>".into()),
            );
            println!(
                "  db_size={:.2} MB, config={}, icons={}",
                manifest.db_size as f64 / 1_048_576.0,
                manifest.has_config,
                manifest.icon_count
            );
            println!();
        }
    }

    // Back up anything that would get overwritten
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    if db_path.exists() {
        let bak = data_dir.join(format!("steam_overachiever.db.bak-import-{}", ts));
        fs::copy(&db_path, &bak)?;
        println!("Backup DB:     {}", bak.display());
    }
    if cfg_path.exists() {
        let bak = data_dir.join(format!("config.toml.bak-import-{}", ts));
        fs::copy(&cfg_path, &bak)?;
        println!("Backup config: {}", bak.display());
    }
    println!();

    // Extract — strict allowlist for paths. No traversal, no symlinks.
    let mut extracted = 0usize;
    let mut skipped = 0usize;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        if name == "manifest.json" {
            continue;
        }
        let rel = match sanitize_entry_name(&name) {
            Some(r) => r,
            None => {
                println!("  ! skipped (unsafe path): {}", name);
                skipped += 1;
                continue;
            }
        };
        let out_path = data_dir.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out_file = File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out_file)?;
        extracted += 1;
    }

    println!();
    println!(
        "Extracted {} file(s){}.",
        extracted,
        if skipped > 0 {
            format!(", skipped {} unsafe", skipped)
        } else {
            String::new()
        }
    );
    println!();
    println!("DB:          {}", db_path.display());
    println!("Config:      {}", cfg_path.display());
    if icon_dir.is_dir() {
        let count = fs::read_dir(&icon_dir)
            .map(|it| it.filter_map(Result::ok).count())
            .unwrap_or(0);
        println!("Icon cache:  {} ({} files)", icon_dir.display(), count);
    }
    println!();
    println!("Next: launch the app. If the cloud token was minted on a different");
    println!("machine it may still be valid; if Steam rejects it with 401, Logout");
    println!("and re-Link in the Profile menu.");
    Ok(())
}

/// Allow only `steam_overachiever.db`, `config.toml`, and files under
/// `icon_cache/`. Reject `..`, absolute paths, and anything else.
fn sanitize_entry_name(name: &str) -> Option<PathBuf> {
    // Normalize separators
    let n = name.replace('\\', "/");
    if n.starts_with('/') || n.contains("..") {
        return None;
    }
    let parts: Vec<&str> = n.split('/').collect();
    match parts.as_slice() {
        ["steam_overachiever.db"] => Some(PathBuf::from("steam_overachiever.db")),
        ["config.toml"] => Some(PathBuf::from("config.toml")),
        ["icon_cache", file] if !file.is_empty() => {
            let mut p = PathBuf::from("icon_cache");
            p.push(file);
            Some(p)
        }
        _ => None,
    }
}
