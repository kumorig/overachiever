# Build script for WASM target
# Prerequisites:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli

$ErrorActionPreference = "Stop"

Write-Host "Building WASM release..." -ForegroundColor Cyan

# Build the wasm crate
cargo build --release --target wasm32-unknown-unknown -p overachiever-wasm

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

Write-Host "Running wasm-bindgen..." -ForegroundColor Cyan

# Create output directory
New-Item -ItemType Directory -Force -Path "web/dist" | Out-Null
New-Item -ItemType Directory -Force -Path "web/dist/pkg" | Out-Null

# Run wasm-bindgen to generate JS bindings
wasm-bindgen --out-dir web/dist/pkg --target web target/wasm32-unknown-unknown/release/overachiever_wasm.wasm

if ($LASTEXITCODE -ne 0) {
    Write-Host "wasm-bindgen failed!" -ForegroundColor Red
    exit 1
}

# Copy index.html to dist
Copy-Item "web/index.html" "web/dist/index.html" -Force

# Copy assets folder if exists
if (Test-Path "assets") {
    Copy-Item -Path "assets" -Destination "web/dist/assets" -Recurse -Force
}

Write-Host ""
Write-Host "Build complete! Files are in web/dist/" -ForegroundColor Green
