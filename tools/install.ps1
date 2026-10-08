[CmdletBinding()]
param(
    [string]$Binary = "",
    [ValidateSet("User", "Machine")]
    [string]$Scope = "User",
    [string]$InstallDir = "",
    [switch]$NoPath
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$global:LASTEXITCODE = 0

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Add-ToPath {
    param(
        [Parameter(Mandatory = $true)][string]$Directory,
        [Parameter(Mandatory = $true)][ValidateSet("User", "Machine")][string]$Target
    )

    $normalized = $Directory.TrimEnd('\')
    $current = [Environment]::GetEnvironmentVariable("Path", $Target)
    $entries = @()

    if (-not [string]::IsNullOrWhiteSpace($current)) {
        $entries = @(
            $current.Split(';', [StringSplitOptions]::RemoveEmptyEntries) |
                ForEach-Object { $_.Trim() } |
                Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        )
    }

    $exists = $entries | Where-Object { $_.TrimEnd('\') -ieq $normalized }
    if (-not $exists) {
        $newPath = (@($entries) + $Directory) -join ';'
        [Environment]::SetEnvironmentVariable("Path", $newPath, $Target)
    }

    if (-not (($env:Path -split ';') | Where-Object { $_.TrimEnd('\') -ieq $normalized })) {
        $env:Path = $env:Path.TrimEnd(';') + ';' + $Directory
    }
}

if ($env:OS -ne "Windows_NT") {
    throw "The current Yad installer supports Windows only."
}

if ($Scope -eq "Machine" -and -not (Test-IsAdministrator)) {
    throw "Machine-wide installation requires an elevated PowerShell session. Use -Scope User or run as Administrator."
}

$ProjectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

if ([string]::IsNullOrWhiteSpace($Binary)) {
    $Binary = Join-Path $ProjectRoot "target\release\yad.exe"
}

$Binary = [IO.Path]::GetFullPath($Binary)
if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
    throw "Yad binary not found: $Binary. Build it first with tools\cargo-msvc.cmd build --release."
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    if ($Scope -eq "Machine") {
        $InstallDir = Join-Path $env:ProgramFiles "Yad\bin"
    }
    else {
        $InstallDir = Join-Path $env:LOCALAPPDATA "Yad\bin"
    }
}

New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

$Destination = Join-Path $InstallDir "yad.exe"
$Staged = Join-Path $InstallDir ("yad.exe.new." + $PID)
$Backup = Join-Path $InstallDir "yad.exe.bak"

Copy-Item -LiteralPath $Binary -Destination $Staged -Force

$versionOutput = & $Staged --version 2>&1
if ($LASTEXITCODE -ne 0) {
    Remove-Item -LiteralPath $Staged -Force -ErrorAction SilentlyContinue
    throw "The staged Yad binary failed to execute: $versionOutput"
}

if (Test-Path -LiteralPath $Destination) {
    Copy-Item -LiteralPath $Destination -Destination $Backup -Force
}

try {
    Move-Item -LiteralPath $Staged -Destination $Destination -Force
}
catch {
    if (Test-Path -LiteralPath $Backup) {
        Copy-Item -LiteralPath $Backup -Destination $Destination -Force -ErrorAction SilentlyContinue
    }
    throw
}

Remove-Item -LiteralPath $Backup -Force -ErrorAction SilentlyContinue

if (-not $NoPath) {
    Add-ToPath -Directory $InstallDir -Target $Scope
}

$installedVersion = & $Destination --version 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "Yad was copied but failed verification: $installedVersion"
}

Write-Host ""
Write-Host "Yad installed successfully." -ForegroundColor Green
Write-Host "Binary : $Destination"
Write-Host "Version: $installedVersion"
Write-Host "PATH   : " -NoNewline
if ($NoPath) {
    Write-Host "not modified"
}
else {
    Write-Host "$Scope PATH contains $InstallDir"
}
Write-Host ""
Write-Host "Try: yad --help"