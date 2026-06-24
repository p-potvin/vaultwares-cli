#!/usr/bin/env pwsh
# VaultWares CLI Multi-Tool Router
# Coordinates and runs workspace operational scripts from a single entrypoint.

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$CommandName,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArgs
)

# Resolve registry path
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RegistryPath = Join-Path $ScriptDir "vw-commands.ps1"

if (-not (Test-Path $RegistryPath)) {
    Write-Error "Command registry not found at: $RegistryPath"
    exit 1
}

# Import commands
$Commands = & $RegistryPath

# Help function
function Show-Help {
    Write-Host "======================================================================" -ForegroundColor Yellow
    Write-Host "                    VAULTWARES COMMAND LINE INTERFACE                 " -ForegroundColor Yellow
    Write-Host "======================================================================" -ForegroundColor Yellow
    Write-Host "Usage: vw <command> [arguments]" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Options:" -ForegroundColor Yellow
    Write-Host "  -h, --help    Show this help menu (or run 'vw <command> -h' for command-specific parameters)" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Available Commands by Category:" -ForegroundColor Yellow
    Write-Host ""

    # Group commands by Category
    $Categories = $Commands.Values | Select-Object -Unique Category | Sort-Object
    foreach ($Cat in $Categories) {
        if (-not $Cat.Category) { continue }
        Write-Host "  [$($Cat.Category)]" -ForegroundColor Yellow
        $CatCmds = $Commands.Keys | Where-Object { $Commands[$_].Category -eq $Cat.Category } | Sort-Object
        foreach ($Cmd in $CatCmds) {
            $Desc = $Commands[$Cmd].Description
            # Formatting line spacing
            $CmdFormatted = $Cmd.PadRight(24)
            Write-Host "    $CmdFormatted : $Desc" -ForegroundColor Gray
        }
        Write-Host ""
    }
    Write-Host "======================================================================" -ForegroundColor Yellow
}

# Check if help is requested
$IsHelp = $false
if ([string]::IsNullOrEmpty($CommandName) -or $CommandName -eq "-h" -or $CommandName -eq "--help" -or $CommandName -eq "help") {
    $IsHelp = $true
}

if ($IsHelp) {
    Show-Help
    exit 0
}

# Verify if command is in registry
if (-not $Commands.Contains($CommandName)) {
    Write-Host "Unknown command: '$CommandName'" -ForegroundColor Red
    Write-Host "Type 'vw --help' to see all available commands." -ForegroundColor Yellow
    exit 1
}

# Get script details
$CmdData = $Commands[$CommandName]
$ScriptPath = $CmdData.ScriptPath

# Check if script exists
if (-not (Test-Path $ScriptPath)) {
    Write-Error "Script file not found for command '$CommandName' at target: $ScriptPath"
    exit 1
}

# Gating mechanism for destructive commands
if ($CmdData.Destructive -eq $true) {
    # 1. AI Assistant refusal (based on ANTIGRAVITY_AGENT, AGENT_NAME, or DEVIN env vars)
    if ($env:ANTIGRAVITY_AGENT -eq "1" -or -not [string]::IsNullOrEmpty($env:AGENT_NAME) -or -not [string]::IsNullOrEmpty($env:DEVIN)) {
        Write-Error "Access Denied: AI assistants are strictly prohibited from executing destructive commands (AI Environment detected)."
        exit 5
    }

    # 2. Strict refusal for non-admin users
    $Identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $Principal = New-Object Security.Principal.WindowsPrincipal($Identity)
    $IsAdmin = $Principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $IsAdmin) {
        Write-Error "Access Denied: Destructive commands can only be executed by a user with Administrator privileges."
        exit 5
    }

    # 3. Confirmation message for Administrator
    Write-Host ""
    Write-Host "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!" -ForegroundColor Red
    Write-Host "  WARNING: Command '$CommandName' is marked as DESTRUCTIVE." -ForegroundColor Red
    Write-Host "  Description: $($CmdData.Description)" -ForegroundColor Yellow
    Write-Host "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!" -ForegroundColor Red
    Write-Host ""
    
    # Check if we are running in non-interactive environment (e.g. piped or redirected stdin)
    if ([Console]::IsInputRedirected) {
        Write-Error "Access Denied: Destructive commands cannot be executed in non-interactive environments (stdin is redirected)."
        exit 5
    }

    $Confirmation = Read-Host "Are you sure you want to proceed? Type 'y' or 'yes' to confirm"
    if ($Confirmation -ne "y" -and $Confirmation -ne "yes") {
        Write-Warning "Execution aborted by user."
        exit 0
    }
}

# Run the command with remaining arguments passed down
Write-Host ">>> Executing command: $CommandName ($($CmdData.Description))" -ForegroundColor Cyan
Write-Host ">>> Path: $ScriptPath" -ForegroundColor DarkGray
Write-Host "----------------------------------------------------------------------" -ForegroundColor DarkGray

# Invoke the target script
if ($RemainingArgs) {
    & $ScriptPath @RemainingArgs
} else {
    & $ScriptPath
}

$ExitCode = $LASTEXITCODE
Write-Host "----------------------------------------------------------------------" -ForegroundColor DarkGray
if ($ExitCode -eq 0) {
    Write-Host ">>> Command completed successfully." -ForegroundColor Green
} else {
    Write-Host ">>> Command completed with exit code: $ExitCode" -ForegroundColor Red
}

exit $ExitCode
