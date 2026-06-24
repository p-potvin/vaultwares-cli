# VaultWares CLI and Docs Rebuilder
# Restores the global vw command-line runner and rebuilds the documentation page resources.

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "=========================================================" -ForegroundColor Yellow
Write-Host "            VAULTWARES CLI & DOCS REBUILDER              " -ForegroundColor Yellow
Write-Host "=========================================================" -ForegroundColor Yellow

# 1. Restore the global shim batch file
Write-Host "[1/4] Ensuring global vw command runner shim exists..." -ForegroundColor Cyan
$LocalBin = "C:\Users\Administrator\.local\bin"
if (-not (Test-Path $LocalBin)) {
    Write-Host "Creating local bin directory at $LocalBin..." -ForegroundColor Gray
    New-Item -ItemType Directory -Path $LocalBin | Out-Null
}

$BatShimPath = Join-Path $LocalBin "vw.bat"
$VwPs1Path = Join-Path $ScriptDir "vw.ps1"

$BatContent = @"
@echo off
rem VaultWares CLI global runner shim
pwsh -NoProfile -ExecutionPolicy Bypass -File "$VwPs1Path" %*
"@

Set-Content -Path $BatShimPath -Value $BatContent -Force
Write-Host "Shim successfully written to $BatShimPath" -ForegroundColor Green

# 2. Check registry files
Write-Host "[2/4] Verifying CLI command registry files..." -ForegroundColor Cyan
$RegistryFile = Join-Path $ScriptDir "vw-commands.ps1"
if (-not (Test-Path $RegistryFile)) {
    Write-Error "Command registry vw-commands.ps1 is missing in $ScriptDir!"
}
if (-not (Test-Path $VwPs1Path)) {
    Write-Error "Main router vw.ps1 is missing in $ScriptDir!"
}
Write-Host "CLI command files validated." -ForegroundColor Green

# 3. Regenerate and rebuild docs
Write-Host "[3/4] Rebuilding vaultwares-docs page resources..." -ForegroundColor Cyan
$DocsDir = "C:\Users\Administrator\Desktop\Github Repos\vaultwares-docs"
if (Test-Path $DocsDir) {
    Push-Location $DocsDir
    try {
        Write-Host "Running page-resources generator..." -ForegroundColor Gray
        npm run generate:page-resources
        
        Write-Host "Compiling production build..." -ForegroundColor Gray
        npm run build
        Write-Host "Docs successfully rebuilt." -ForegroundColor Green
    }
    finally {
        Pop-Location
    }
} else {
    Write-Host "Warning: vaultwares-docs directory not found. Skipping docs build." -ForegroundColor Yellow
}

# 4. Verify CLI runs
Write-Host "[4/4] Verifying CLI integration..." -ForegroundColor Cyan
try {
    # Test execution
    & $BatShimPath --help | Out-Null
    Write-Host "Global vw CLI validated and operational!" -ForegroundColor Green
}
catch {
    Write-Error "Failed to run global 'vw' command: $_"
}

Write-Host "=========================================================" -ForegroundColor Yellow
Write-Host " Rebuild Completed Successfully!                         " -ForegroundColor Green
Write-Host "=========================================================" -ForegroundColor Yellow
