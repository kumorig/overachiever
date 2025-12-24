use std::fs;
use std::path::Path;

fn main() {
    // Generate build info
    generate_build_info();
    
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon.ico");
        res.compile().unwrap();
    }
}

fn generate_build_info() {
    let build_info_path = Path::new("../../build_info.json");
    
    // Read existing build number or start at 0
    let build_number: u32 = if build_info_path.exists() {
        if let Ok(content) = fs::read_to_string(build_info_path) {
            // Simple JSON parsing - look for "build_number": N
            content
                .lines()
                .find(|line| line.contains("build_number"))
                .and_then(|line| {
                    line.split(':')
                        .nth(1)
                        .map(|s| s.trim().trim_matches(',').trim().parse().unwrap_or(0))
                })
                .unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };
    
    // Get build datetime
    let build_datetime = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    
    // Pass to compiler
    println!("cargo:rustc-env=BUILD_NUMBER={}", build_number);
    println!("cargo:rustc-env=BUILD_DATETIME={}", build_datetime);
    
    // Rerun if build_info.json changes (so we pick up new build numbers from WASM builds)
    println!("cargo:rerun-if-changed=../../build_info.json");
}
