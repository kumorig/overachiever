# Server setup script for Overachiever on tatsugo
# Run this once to set up nginx config, systemd service, and SSL

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Overachiever Server Setup" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

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
$setupCommands = @"
# Create directories
sudo mkdir -p /var/www/overachiever
sudo mkdir -p /opt/overachiever
sudo chown -R www-data:www-data /var/www/overachiever

# Install nginx config
sudo mv /tmp/nginx-overachiever.conf /etc/nginx/sites-available/overachiever.space
sudo ln -sf /etc/nginx/sites-available/overachiever.space /etc/nginx/sites-enabled/

# Install systemd service
sudo mv /tmp/overachiever-backend.service /etc/systemd/system/
sudo systemctl daemon-reload

# Create env file if not exists
if [ ! -f /opt/overachiever/.env ]; then
    sudo mv /tmp/overachiever.env.example /opt/overachiever/.env
    echo "Created /opt/overachiever/.env - PLEASE EDIT WITH REAL VALUES"
else
    rm /tmp/overachiever.env.example
    echo "Keeping existing /opt/overachiever/.env"
fi

# Test nginx config
sudo nginx -t

echo ""
echo "Setup complete! Next steps:"
echo "1. Edit /opt/overachiever/.env with real database credentials"
echo "2. Run: sudo certbot --nginx -d overachiever.space -d www.overachiever.space"
echo "3. Run: sudo systemctl enable overachiever-backend"
echo "4. Deploy with: npm run deploy"
"@

plink tatsugo $setupCommands

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
