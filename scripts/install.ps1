#Requires -Version 5.1

param(
    [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"

# Configuration
$Repo = "goudyj/assistant-cli"
$BinaryName = "assistant.exe"
$InstallDir = "$env:LOCALAPPDATA\Programs\assistant"
$ConfigDir = "$env:USERPROFILE\.config"
$ConfigFile = "$ConfigDir\assistant.json"

# Default GitHub Client ID (public identifier, not a secret)
$DefaultGitHubClientId = "Ov23li3PDrRNh2FnCku1"

function Write-Info { param($Message) Write-Host "[INFO] $Message" -ForegroundColor Green }
function Write-Warn { param($Message) Write-Host "[WARN] $Message" -ForegroundColor Yellow }
function Write-Err { param($Message) Write-Host "[ERROR] $Message" -ForegroundColor Red; exit 1 }

function Get-LatestVersion {
    param([string]$RequestedVersion)

    if ($RequestedVersion -eq "latest") {
        try {
            $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
            return $release.tag_name
        }
        catch {
            Write-Err "Failed to fetch latest version. Check your internet connection."
        }
    }
    return $RequestedVersion
}

function Install-Binary {
    param([string]$Version)

    $AssetName = "assistant-x86_64-pc-windows-msvc.exe"
    $DownloadUrl = "https://github.com/$Repo/releases/download/$Version/$AssetName"

    Write-Info "Downloading $AssetName ($Version)..."

    # Create install directory
    if (!(Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Download binary
    $DestPath = "$InstallDir\$BinaryName"
    try {
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $DestPath -UseBasicParsing
    }
    catch {
        Write-Err "Failed to download from $DownloadUrl"
    }

    Write-Info "Installed to $DestPath"
}

function Add-ToPath {
    $CurrentPath = [Environment]::GetEnvironmentVariable("Path", "User")

    if ($CurrentPath -notlike "*$InstallDir*") {
        Write-Info "Adding $InstallDir to PATH..."
        $NewPath = "$InstallDir;$CurrentPath"
        [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
        $env:Path = "$InstallDir;$env:Path"
        Write-Info "PATH updated. Restart your terminal to use 'assistant' command."
    }
    else {
        Write-Info "$InstallDir is already in PATH"
    }
}

function New-DefaultConfig {
    if (Test-Path $ConfigFile) {
        Write-Info "Config file already exists at $ConfigFile"
        return
    }

    Write-Info "Creating default config file..."

    if (!(Test-Path $ConfigDir)) {
        New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
    }

    $ConfigContent = @"
{
  "github_client_id": "$DefaultGitHubClientId",
  "projects": {}
}
"@

    Set-Content -Path $ConfigFile -Value $ConfigContent -Encoding UTF8
    Write-Info "Created config at $ConfigFile"
}

# Main
Write-Info "Installing assistant..."

$Version = Get-LatestVersion -RequestedVersion $Version
Write-Info "Version: $Version"

Install-Binary -Version $Version
New-DefaultConfig
Add-ToPath

Write-Host ""
Write-Info "Installation complete!"
Write-Info "Run 'assistant' to get started."
