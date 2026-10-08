# Yad bootstrap installer for Windows.
#
# One-line GitHub usage:
#   irm https://raw.githubusercontent.com/akbaramd/Yad/main/install.ps1 | iex
#
# Override the repository for testing or forks with:
#   $env:YAD_GITHUB_REPOSITORY = "OWNER/REPO"
#
# Expected GitHub Release assets:
#   yad-windows-x64.zip
#   yad-windows-x64.zip.sha256
#
# Optional model assets:
#   yad-model-multilingual-e5-small-int8.zip
#   yad-model-multilingual-e5-small-int8.zip.sha256

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$global:LASTEXITCODE = 0

$Repository = if (-not [string]::IsNullOrWhiteSpace($env:YAD_GITHUB_REPOSITORY)) {
    $env:YAD_GITHUB_REPOSITORY
}
else {
    "akbaramd/Yad"
}

$InstallDir = Join-Path $env:LOCALAPPDATA "Yad\bin"
$AssetName = "yad-windows-x64.zip"
$ChecksumAssetName = "$AssetName.sha256"
$ModelAssetName = "yad-model-multilingual-e5-small-int8.zip"
$ModelChecksumAssetName = "$ModelAssetName.sha256"

function Add-UserPath {
    param([Parameter(Mandatory = $true)][string]$Directory)

    $normalized = $Directory.TrimEnd('\')
    $current = [Environment]::GetEnvironmentVariable("Path", "User")
    $entries = @()

    if (-not [string]::IsNullOrWhiteSpace($current)) {
        $entries = @(
            $current.Split(';', [StringSplitOptions]::RemoveEmptyEntries) |
                ForEach-Object { $_.Trim() } |
                Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        )
    }

    if (-not ($entries | Where-Object { $_.TrimEnd('\') -ieq $normalized })) {
        [Environment]::SetEnvironmentVariable(
            "Path",
            ((@($entries) + $Directory) -join ';'),
            "User"
        )
    }

    if (-not (($env:Path -split ';') | Where-Object { $_.TrimEnd('\') -ieq $normalized })) {
        $env:Path = $env:Path.TrimEnd(';') + ';' + $Directory
    }
}

function Get-ReleaseAsset {
    param(
        [Parameter(Mandatory = $true)]$Release,
        [Parameter(Mandatory = $true)][string]$Name,
        [switch]$Optional
    )

    $asset = $Release.assets | Where-Object { $_.name -eq $Name } | Select-Object -First 1
    if (-not $asset -and -not $Optional) {
        throw "GitHub Release asset '$Name' was not found."
    }
    return $asset
}

function Download-File {
    param(
        [Parameter(Mandatory = $true)][string]$Url,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $Destination -Headers @{
        "User-Agent" = "Yad-Installer"
        "Accept" = "application/octet-stream"
    }
}

function Test-Checksum {
    param(
        [Parameter(Mandatory = $true)][string]$File,
        [Parameter(Mandatory = $true)][string]$ChecksumFile
    )

    $expectedLine = (Get-Content -LiteralPath $ChecksumFile -Raw).Trim()
    $expected = ($expectedLine -split '\s+')[0].Trim().ToLowerInvariant()
    $actual = (Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLowerInvariant()

    if ($expected -ne $actual) {
        throw "SHA-256 verification failed for $(Split-Path -Leaf $File). Expected $expected, got $actual."
    }
}

function Ensure-Docker {
    $docker = Get-Command docker -ErrorAction SilentlyContinue
    if ($docker) {
        return $true
    }

    Write-Host "Docker was not found. Yad uses Docker for local Qdrant." -ForegroundColor Yellow

    $winget = Get-Command winget -ErrorAction SilentlyContinue
    if (-not $winget) {
        Write-Host "winget is not available, so Docker Desktop cannot be installed automatically." -ForegroundColor Yellow
        Write-Host "Install Docker Desktop manually before running 'yad infra up'." -ForegroundColor Yellow
        return $false
    }

    Write-Host "Installing Docker Desktop with winget..." -ForegroundColor Cyan
    & winget install --id Docker.DockerDesktop -e --accept-package-agreements --accept-source-agreements --silent --disable-interactivity

    if ($LASTEXITCODE -ne 0) {
        Write-Host "Docker Desktop installation did not complete successfully. Yad itself is still installed." -ForegroundColor Yellow
        return $false
    }

    Write-Host "Docker Desktop was installed. A sign-out/restart or opening Docker Desktop may be required before Qdrant can start." -ForegroundColor Yellow
    return $false
}

if ($env:OS -ne "Windows_NT") {
    throw "This bootstrap currently supports Windows only."
}

$arch = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
if ($arch -ne "X64") {
    throw "No published Yad bootstrap package is configured for architecture '$arch' yet."
}

[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$TempRoot = Join-Path ([IO.Path]::GetTempPath()) ("yad-install-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $TempRoot -Force | Out-Null

try {
    Write-Host "Resolving latest Yad release..." -ForegroundColor Cyan
    try {
        $release = Invoke-RestMethod -UseBasicParsing -Uri "https://api.github.com/repos/$Repository/releases/latest" -Headers @{ "User-Agent" = "Yad-Installer" }
    }
    catch {
        throw "No published GitHub Release was found for $Repository. Install from source for now: https://github.com/$Repository"
    }

    $binaryAsset = Get-ReleaseAsset -Release $release -Name $AssetName
    $checksumAsset = Get-ReleaseAsset -Release $release -Name $ChecksumAssetName

    $zipPath = Join-Path $TempRoot $AssetName
    $shaPath = Join-Path $TempRoot $ChecksumAssetName

    Write-Host "Downloading $AssetName..." -ForegroundColor Cyan
    Download-File -Url $binaryAsset.browser_download_url -Destination $zipPath
    Download-File -Url $checksumAsset.browser_download_url -Destination $shaPath
    Test-Checksum -File $zipPath -ChecksumFile $shaPath

    $extractPath = Join-Path $TempRoot "binary"
    Expand-Archive -LiteralPath $zipPath -DestinationPath $extractPath -Force

    $sourceExe = Get-ChildItem -LiteralPath $extractPath -Filter "yad.exe" -File -Recurse | Select-Object -First 1
    if (-not $sourceExe) {
        throw "The release package does not contain yad.exe."
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $destination = Join-Path $InstallDir "yad.exe"
    $staged = Join-Path $InstallDir ("yad.exe.new." + $PID)

    Copy-Item -LiteralPath $sourceExe.FullName -Destination $staged -Force

    $stageVersion = & $staged --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Downloaded Yad binary failed verification: $stageVersion"
    }

    Move-Item -LiteralPath $staged -Destination $destination -Force
    Add-UserPath -Directory $InstallDir

    Write-Host "Installed Yad: $stageVersion" -ForegroundColor Green

    $modelAsset = Get-ReleaseAsset -Release $release -Name $ModelAssetName -Optional
    $modelChecksumAsset = Get-ReleaseAsset -Release $release -Name $ModelChecksumAssetName -Optional

    if ($modelAsset -and $modelChecksumAsset) {
        $modelZip = Join-Path $TempRoot $ModelAssetName
        $modelSha = Join-Path $TempRoot $ModelChecksumAssetName
        $modelExtract = Join-Path $TempRoot "model"

        Write-Host "Downloading local embedding model..." -ForegroundColor Cyan
        Download-File -Url $modelAsset.browser_download_url -Destination $modelZip
        Download-File -Url $modelChecksumAsset.browser_download_url -Destination $modelSha
        Test-Checksum -File $modelZip -ChecksumFile $modelSha

        Expand-Archive -LiteralPath $modelZip -DestinationPath $modelExtract -Force

        $modelFolder = Get-ChildItem -LiteralPath $modelExtract -Directory -Recurse |
            Where-Object { Test-Path (Join-Path $_.FullName "model.onnx") } |
            Select-Object -First 1

        if (-not $modelFolder -and (Test-Path (Join-Path $modelExtract "model.onnx"))) {
            $modelFolder = Get-Item $modelExtract
        }

        if (-not $modelFolder) {
            throw "The model release asset does not contain the expected model bundle."
        }

        & $destination model install --from $modelFolder.FullName
        if ($LASTEXITCODE -ne 0) {
            throw "Yad model installation failed."
        }
    }
    else {
        Write-Host "No model bundle was attached to this release. Yad can bootstrap the pinned model later when semantic indexing is first used." -ForegroundColor Yellow
    }

    $dockerReady = Ensure-Docker

    if ($dockerReady) {
        Write-Host "Starting local Qdrant..." -ForegroundColor Cyan
        & $destination infra up
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Yad installed, but Qdrant did not start. Run 'yad infra up' after Docker is ready." -ForegroundColor Yellow
        }
    }

    Write-Host ""
    Write-Host "Yad installation complete." -ForegroundColor Green
    Write-Host "Installed to: $destination"
    Write-Host "Try:"
    Write-Host "  yad --help"
    Write-Host "  yad model status"
    Write-Host "  yad infra status"
}
finally {
    Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
