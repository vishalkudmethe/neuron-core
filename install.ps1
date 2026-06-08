# Neuron v16 - Windows Global Installer
# Run from repository root: .\install.ps1

$ErrorActionPreference = "Stop"

$BinaryName   = "neuron.exe"
$SourceBinary = Join-Path $PSScriptRoot "target\release\$BinaryName"
$InstallDir   = Join-Path $env:USERPROFILE "bin"
$InstallPath  = Join-Path $InstallDir $BinaryName

# 1. Verify binary exists
if (-not (Test-Path $SourceBinary)) {
    Write-Host "  [!] Release binary not found at $SourceBinary" -ForegroundColor Red
    Write-Host "      Run: cargo build --release" -ForegroundColor Yellow
    exit 1
}

# 2. Ensure install directory exists
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
    Write-Host "  [+] Created install directory: $InstallDir" -ForegroundColor Cyan
}

# 3. Copy binary
Copy-Item -Path $SourceBinary -Destination $InstallPath -Force
$SizeKB = [math]::Round((Get-Item $InstallPath).Length / 1KB)
Write-Host "  [+] Installed: $InstallPath ($SizeKB KB)" -ForegroundColor Green

# 4. Register in PATH if not already present
$CurrentPath = [System.Environment]::GetEnvironmentVariable("PATH", "User")
if ($CurrentPath -notlike "*$InstallDir*") {
    [System.Environment]::SetEnvironmentVariable(
        "PATH",
        "$CurrentPath;$InstallDir",
        "User"
    )
    Write-Host "  [+] Added $InstallDir to User PATH (restart terminal to activate)" -ForegroundColor Cyan
} else {
    Write-Host "  [i] $InstallDir already in PATH" -ForegroundColor DarkCyan
}

# 5. Sanity check from temp directory
Write-Host ""
Write-Host "  Running sanity check..." -ForegroundColor DarkGray
$env:PATH = "$env:PATH;$InstallDir"
$VersionOutput = & $InstallPath "--version" 2>&1
Write-Host "  [x] $VersionOutput" -ForegroundColor Green
Write-Host ""
Write-Host "  ================================================" -ForegroundColor Cyan
Write-Host "  Neuron v16 successfully installed and operational." -ForegroundColor White
Write-Host "  Run 'neuron --help' from any directory." -ForegroundColor DarkCyan
Write-Host "  ================================================" -ForegroundColor Cyan
Write-Host ""
