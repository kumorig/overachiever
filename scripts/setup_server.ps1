# Server setup script for Overachiever on tatsugo
# Run this once to set up Rust, nginx config, systemd service, and SSL

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Overachiever Server Setup" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if Rust is installed
Write-Host "Checking if Rust is installed on server..." -ForegroundColor Yellow
$rustCheck = plink tatsugo "which cargo 2>/dev/null && echo RUST_OK || echo RUST_MISSING"

if ($rustCheck -match "RUST_MISSING") {
    Write-Host "Rust not found. Installing Rust on server..." -ForegroundColor Yellow
    plink tatsugo "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source ~/.cargo/env && cargo --version"
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Rust installation failed!" -ForegroundColor Red
        exit 1
    }
    Write-Host "Rust installed successfully!" -ForegroundColor Green
} else {
    Write-Host "Rust is already installed." -ForegroundColor Green
}

# Copy config files to server
Write-Host "Copying config files to server..." -ForegroundColor Yellow
pscp "$ScriptDir\server\nginx-overachiever.conf" "tatsugo:/tmp/"
pscp "$ScriptDir\server\overachiever-backend.service" "tatsugo:/tmp/"
pscp "$ScriptDir\server\overachiever.env.example" "tatsugo:/tmp/"

if ($LASTEXITCODE -ne 0) {
    Write-Host "File copy failed!" -ForegroundColor Red
    exit 1
}

# Set up server
Write-Host "Setting up server..." -ForegroundColor Yellow

# Run commands one by one to avoid line ending issues
plink tatsugo "sudo mkdir -p /var/www/overachiever /opt/overachiever /opt/overachiever/src"
plink tatsugo "sudo chown -R `$USER:`$USER /opt/overachiever"
plink tatsugo "sudo chown -R www-data:www-data /var/www/overachiever"
plink tatsugo "sudo mv /tmp/nginx-overachiever.conf /etc/nginx/sites-available/overachiever.space"
plink tatsugo "sudo ln -sf /etc/nginx/sites-available/overachiever.space /etc/nginx/sites-enabled/"
plink tatsugo "sudo mv /tmp/overachiever-backend.service /etc/systemd/system/"
plink tatsugo "sudo systemctl daemon-reload"
plink tatsugo "test -f /opt/overachiever/.env || (sudo mv /tmp/overachiever.env.example /opt/overachiever/.env && sudo chown `$USER:`$USER /opt/overachiever/.env && echo 'Created .env')"
plink tatsugo "rm -f /tmp/overachiever.env.example 2>/dev/null || true"
plink tatsugo "sudo nginx -t"

if ($LASTEXITCODE -ne 0) {
    Write-Host "Setup failed!" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Server setup complete!" -ForegroundColor Green
Write-Host ""
Write-Host "Don't forget to:" -ForegroundColor Yellow
Write-Host "  1. Edit /opt/overachiever/.env on the server with real credentials" -ForegroundColor Yellow
Write-Host "  2. Run certbot for SSL: sudo certbot --nginx -d overachiever.space" -ForegroundColor Yellow
Write-Host "  3. Enable the backend service: sudo systemctl enable overachiever-backend" -ForegroundColor Yellow
