# Rust Learning App - Podman helper (PowerShell 5.1)
param(
    [switch]$Build,
    [switch]$Up,
    [switch]$Down,
    [switch]$Logs,
    [switch]$Check,
    [switch]$Shell
)
$ErrorActionPreference = 'Stop'
$env:PODMAN_COMPOSE_WARNING_LOGS = 'false'
Set-Location -Path $PSScriptRoot

if (-not (Test-Path 'data')) { New-Item -ItemType Directory -Path 'data' | Out-Null }

function Test-PodmanCompose {
    if (-not (Get-Command podman -ErrorAction SilentlyContinue)) { return $false }
    $old = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        & cmd /c "podman compose version >nul 2>&1"
        return ($LASTEXITCODE -eq 0)
    } catch {
        return $false
    } finally {
        $ErrorActionPreference = $old
    }
}

function Invoke-Compose {
    param([string[]]$CmdArgs)
    $useBuiltin = $false
    $ErrorActionPreference = 'Continue'
    if (Test-PodmanCompose) { $useBuiltin = $true }
    if ($useBuiltin) {
        & podman compose @CmdArgs
    } elseif (Get-Command podman-compose -ErrorAction SilentlyContinue) {
        & podman-compose @CmdArgs
    } else {
        throw 'Neither "podman compose" nor "podman-compose" was found. Install Podman Desktop.'
    }
    if ($LASTEXITCODE -ne 0) { throw "compose failed (exit $LASTEXITCODE)" }
}

$any = $Build -or $Up -or $Down -or $Logs -or $Check -or $Shell
if (-not $any) { $Build = $true; $Up = $true }

if ($Down)  { Invoke-Compose @('down') }
function Build-Image {
    $ErrorActionPreference = 'Continue'
    & podman build -f Containerfile -t rust-learning-app:latest .
    if ($LASTEXITCODE -ne 0) { throw "build failed (exit $LASTEXITCODE)" }
}

if ($Build) { Build-Image }
if ($Check) { Invoke-Compose @('run', '--rm', 'app', '/app/rust_learning_app', '--check-content') }
if ($Shell) { Invoke-Compose @('run', '--rm', '--entrypoint', '/bin/bash', 'app') }
if ($Up) {
    $ErrorActionPreference = 'Continue'
    & podman image exists rust-learning-app:latest
    if ($LASTEXITCODE -ne 0) { Build-Image }
    Invoke-Compose @('up', '-d')
    Write-Host 'http://localhost:8080'
}
if ($Logs)  { Invoke-Compose @('logs', '-f', 'app') }
