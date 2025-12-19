# Build script for Linux backend (cross-compilation)
# Prerequisites:
#   rustup target add x86_64-unknown-linux-gnu
#   Install cross: cargo install cross

$ErrorActionPreference = "Stop"

Write-Host "Building backend for Linux..." -ForegroundColor Cyan

# Use cross for Linux cross-compilation from Windows
cross build --release --target x86_64-unknown-linux-gnu -p overachiever-backend

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Build complete! Binary at: target/x86_64-unknown-linux-gnu/release/overachiever-server" -ForegroundColor Green
