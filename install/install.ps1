# TUIQL PowerShell Installation Script
# This script automatically downloads and installs TUIQL on Windows

param(
    [string]$Version = "0.1.0",
    [string]$InstallDir = "$env:ProgramFiles\TUIQL",
    [switch]$AddToPath
)

$ErrorActionPreference = "Stop"

# Configuration
$Repo = "tuiql/tuiql"
$BaseUrl = "https://github.com/$Repo/releases/download"
$BinaryName = "tuiql.exe"

# Console colors for better output
$Green = "Green"
$Yellow = "Yellow"
$Red = "Red"

function Write-Info {
    param([string]$Message)
    Write-Host "INFO: $Message" -ForegroundColor $Green
}

function Write-Warn {
    param([string]$Message)
    Write-Host "WARN: $Message" -ForegroundColor $Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "ERROR: $Message" -ForegroundColor $Red
}

function Test-Admin {
    $currentUser = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($currentUser)
    return $principal.IsInRole([Security.Principal.WindowsBuiltinRole]::Administrator)
}

function Get-WindowsArch {
    $arch = $env:PROCESSOR_ARCHITECTURE
    switch ($arch) {
        "AMD64" { return "x86_64" }
        "ARM64" { return "aarch64" }
        default { throw "Unsupported architecture: $arch" }
    }
}

function Download-Binary {
    $arch = Get-WindowsArch
    $target = "$arch-pc-windows-msvc"
    $url = "$BaseUrl/v$Version/tuiql-v$Version-$target.tar.gz"

    Write-Info "Downloading TUIQL v$Version for Windows ($arch)..."

    $tempDir = New-TemporaryFile | %{ Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    $archivePath = Join-Path $tempDir "tuiql.tar.gz"

    try {
        Invoke-WebRequest -Uri $url -OutFile $archivePath -UseBasicParsing

        Write-Info "Download completed. Extracting..."

        # Extract tar.gz (requires tar or 7zip)
        $extractPath = Join-Path $tempDir "extracted"
        New-Item -ItemType Directory -Path $extractPath | Out-Null

        if (Get-Command "tar" -ErrorAction SilentlyContinue) {
            & tar -xzf $archivePath -C $extractPath
        } elseif (Get-Command "7z" -ErrorAction SilentlyContinue) {
            & 7z x $archivePath -o"$extractPath" | Out-Null
            & 7z x (Join-Path $extractPath "*.tar") -o"$extractPath" | Out-Null
        } else {
            throw "No extraction tool found. Please install tar or 7-Zip."
        }

        $binaryPath = Get-ChildItem -Path $extractPath -Filter $BinaryName -Recurse | Select-Object -First 1
        if (!$binaryPath) {
            throw "Binary not found in extracted archive"
        }

        # Install binary
        Write-Info "Installing to $InstallDir..."
        if (!(Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir | Out-Null
        }

        Copy-Item $binaryPath.FullName -Destination (Join-Path $InstallDir $BinaryName) -Force

        Write-Info "Installation completed."

    } finally {
        Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Add-ToPath {
    if (!$AddToPath) { return }

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -notlike "*$InstallDir*") {
        $newPath = "$currentPath;$InstallDir"
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        Write-Info "Added TUIQL to PATH. Restart your terminal to use 'tuiql' command."
    }
}

function Test-Installation {
    $tuiqlPath = Join-Path $InstallDir $BinaryName

    if (!(Test-Path $tuiqlPath)) {
        throw "Installation verification failed: $tuiqlPath not found"
    }

    Write-Info "Testing installation..."
    & $tuiqlPath "--version"

    if ($LASTEXITCODE -ne 0) {
        throw "Installation verification failed: binary returned exit code $LASTEXITCODE"
    }

    Write-Info "Installation verified successfully!"
}

function Main {
    Write-Info "TUIQL PowerShell Installer v$Version"
    Write-Info "Repository: $Repo"

    # Check if running as admin if needed
    if (!(Test-Admin) -and !(Test-Path $InstallDir -PathType Container -ErrorAction SilentlyContinue)) {
        Write-Warn "You may need to run as Administrator for the first installation."
    }

    Download-Binary
    Add-ToPath
    Test-Installation

    Write-Info ""
    Write-Info "Installation completed successfully!"
    Write-Info "Run '$InstallDir\$BinaryName --help' to get started."

    if (!$AddToPath) {
        Write-Info "Tip: Add '$InstallDir' to your PATH to use 'tuiql' from anywhere."
    }
}

try {
    Main
} catch {
    Write-Error "Installation failed: $($_.Exception.Message)"
    Write-Error "Please check your internet connection and try again."
    exit 1
}