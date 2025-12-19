# Deployment script for Overachiever
# Builds WASM locally, syncs source to server, builds backend on server

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

Push-Location $ProjectRoot

try {
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host " Overachiever Deployment" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""

    # Step 1: Build WASM locally
    Write-Host "Step 1: Building WASM locally..." -ForegroundColor Yellow
    & "$ScriptDir\build_wasm.ps1"

    if ($LASTEXITCODE -ne 0) {
        Write-Host "WASM build failed! Aborting deployment." -ForegroundColor Red
        exit 1
    }

    Write-Host ""
    Write-Host "Step 2: Deploying to tatsugo..." -ForegroundColor Yellow

    # Remote paths
    $remoteWebPath = "/var/www/overachiever"
    $remoteBackendPath = "/opt/overachiever"
    $remoteSrcPath = "/opt/overachiever/src"

    # Create remote directories
    Write-Host "Creating remote directories..." -ForegroundColor Cyan
    plink tatsugo "sudo mkdir -p $remoteWebPath && sudo mkdir -p $remoteBackendPath && sudo mkdir -p $remoteSrcPath && mkdir -p /tmp/overachiever_web"

    # Deploy WASM frontend
    Write-Host "Copying WASM files..." -ForegroundColor Cyan
    pscp -r web/dist/* "tatsugo:/tmp/overachiever_web/"

    if ($LASTEXITCODE -ne 0) {
        Write-Host "WASM file copy failed!" -ForegroundColor Red
        exit 1
    }

    # Sync backend source code to server (only crates needed for backend)
    Write-Host "Syncing backend source code..." -ForegroundColor Cyan
    # Use server-specific Cargo.toml that only includes core and backend
    pscp "$ScriptDir\server\Cargo.server.toml" "tatsugo:$remoteSrcPath/Cargo.toml"
    pscp "Cargo.lock" "tatsugo:$remoteSrcPath/"
    plink tatsugo "mkdir -p $remoteSrcPath/crates"
    pscp -r crates/core "tatsugo:$remoteSrcPath/crates/"
    pscp -r crates/backend "tatsugo:$remoteSrcPath/crates/"

    if ($LASTEXITCODE -ne 0) {
        Write-Host "Source sync failed!" -ForegroundColor Red
        exit 1
    }

    # Build backend on server and deploy
    Write-Host "Building backend on server (this may take a while on first run)..." -ForegroundColor Cyan
    
    # Source cargo env and build
    plink tatsugo "source ~/.cargo/env && cd $remoteSrcPath && cargo build --release -p overachiever-backend"
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Backend build failed!" -ForegroundColor Red
        exit 1
    }
    
    Write-Host "Deploying files..." -ForegroundColor Cyan
    
    # Deploy WASM files
    plink tatsugo "sudo rm -rf $remoteWebPath/*"
    plink tatsugo "sudo mv /tmp/overachiever_web/* $remoteWebPath/"
    plink tatsugo "sudo rmdir /tmp/overachiever_web 2>/dev/null || true"
    plink tatsugo "sudo chown -R www-data:www-data $remoteWebPath"
    
    # Deploy backend binary
    plink tatsugo "sudo cp $remoteSrcPath/target/release/overachiever-server $remoteBackendPath/"
    plink tatsugo "sudo chmod +x $remoteBackendPath/overachiever-server"
    
    # Restart backend service
    plink tatsugo "sudo systemctl restart overachiever-backend 2>/dev/null || echo 'Service not yet running'"
    
    # Reload nginx
    plink tatsugo "sudo nginx -t && sudo systemctl reload nginx"

    if ($LASTEXITCODE -ne 0) {
        Write-Host "Build/deployment on server failed!" -ForegroundColor Red
        exit 1
    }

    Write-Host ""
    Write-Host "========================================" -ForegroundColor Green
    Write-Host " Deployment complete!" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Web app:  https://overachiever.space/" -ForegroundColor Green
    Write-Host "Backend:  https://overachiever.space/ws" -ForegroundColor Green
    Write-Host ""
}
finally {
    Pop-Location
}
