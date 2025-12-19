# Deployment script for Overachiever
# Builds WASM and backend locally, deploys to tatsugo server

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

Push-Location $ProjectRoot

try {
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host " Overachiever Deployment" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""

    # Step 1: Build WASM
    Write-Host "Step 1: Building WASM..." -ForegroundColor Yellow
    & "$ScriptDir\build_wasm.ps1"

    if ($LASTEXITCODE -ne 0) {
        Write-Host "WASM build failed! Aborting deployment." -ForegroundColor Red
        exit 1
    }

    Write-Host ""

    # Step 2: Build backend
    Write-Host "Step 2: Building backend for Linux..." -ForegroundColor Yellow
    & "$ScriptDir\build_backend.ps1"

    if ($LASTEXITCODE -ne 0) {
        Write-Host "Backend build failed! Aborting deployment." -ForegroundColor Red
        exit 1
    }

    Write-Host ""
    Write-Host "Step 3: Deploying to tatsugo..." -ForegroundColor Yellow

    # Remote paths
    $remoteWebPath = "/var/www/overachiever"
    $remoteBackendPath = "/opt/overachiever"

    # Create remote directories
    Write-Host "Creating remote directories..." -ForegroundColor Cyan
    plink tatsugo "sudo mkdir -p $remoteWebPath && sudo mkdir -p $remoteBackendPath && mkdir -p /tmp/overachiever_web && mkdir -p /tmp/overachiever_backend"

    # Deploy WASM frontend
    Write-Host "Copying WASM files..." -ForegroundColor Cyan
    pscp -r web/dist/* "tatsugo:/tmp/overachiever_web/"

    if ($LASTEXITCODE -ne 0) {
        Write-Host "WASM file copy failed!" -ForegroundColor Red
        exit 1
    }

    # Deploy backend binary
    Write-Host "Copying backend binary..." -ForegroundColor Cyan
    pscp "target/x86_64-unknown-linux-gnu/release/overachiever-server" "tatsugo:/tmp/overachiever_backend/"

    if ($LASTEXITCODE -ne 0) {
        Write-Host "Backend file copy failed!" -ForegroundColor Red
        exit 1
    }

    # Move files and restart services
    Write-Host "Moving files and restarting services..." -ForegroundColor Cyan
    $deployCommands = @"
sudo rm -rf $remoteWebPath/*
sudo mv /tmp/overachiever_web/* $remoteWebPath/
sudo rmdir /tmp/overachiever_web
sudo mv /tmp/overachiever_backend/overachiever-server $remoteBackendPath/
sudo chmod +x $remoteBackendPath/overachiever-server
sudo rmdir /tmp/overachiever_backend
sudo chown -R www-data:www-data $remoteWebPath
sudo systemctl restart overachiever-backend || true
sudo nginx -t && sudo systemctl reload nginx
"@

    plink tatsugo $deployCommands

    if ($LASTEXITCODE -ne 0) {
        Write-Host "Deployment failed!" -ForegroundColor Red
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
