[CmdletBinding()]
param(
    [string]$OutputDir = "",
    [switch]$SkipBuild,
    [switch]$IncludeModel
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = Join-Path $ProjectRoot "dist"
}
elseif (-not [IO.Path]::IsPathRooted($OutputDir)) {
    $OutputDir = Join-Path $ProjectRoot $OutputDir
}

if (-not $SkipBuild) {
    Write-Host "Building release binary..." -ForegroundColor Cyan
    & (Join-Path $ProjectRoot "tools\cargo-msvc.cmd") build --release
    if ($LASTEXITCODE -ne 0) {
        throw "Release build failed."
    }
}

$Binary = Join-Path $ProjectRoot "target\release\yad.exe"
if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
    throw "Release binary not found: $Binary"
}

$VersionOutput = (& $Binary --version 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0) {
    throw "Could not read Yad version: $VersionOutput"
}

New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null

$TempRoot = Join-Path ([IO.Path]::GetTempPath()) ("yad-package-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $TempRoot -Force | Out-Null

function Write-Checksum {
    param([Parameter(Mandatory = $true)][string]$File)

    $hash = (Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLowerInvariant()
    $name = Split-Path -Leaf $File
    $checksumPath = "$File.sha256"
    $line = $hash + "  " + $name + [Environment]::NewLine
    [IO.File]::WriteAllText(
        $checksumPath,
        $line,
        (New-Object Text.UTF8Encoding($false))
    )
    return $checksumPath
}

try {
    $binaryStage = Join-Path $TempRoot "binary"
    New-Item -ItemType Directory -Path $binaryStage -Force | Out-Null

    Copy-Item -LiteralPath $Binary -Destination (Join-Path $binaryStage "yad.exe") -Force
    Copy-Item -LiteralPath (Join-Path $ProjectRoot "README.md") -Destination (Join-Path $binaryStage "README.md") -Force

    $BinaryZip = Join-Path $OutputDir "yad-windows-x64.zip"
    Remove-Item -LiteralPath $BinaryZip -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath "$BinaryZip.sha256" -Force -ErrorAction SilentlyContinue

    Compress-Archive -Path (Join-Path $binaryStage "*") -DestinationPath $BinaryZip -CompressionLevel Optimal
    $BinaryChecksum = Write-Checksum -File $BinaryZip

    Write-Host "Created: $BinaryZip" -ForegroundColor Green
    Write-Host "Created: $BinaryChecksum" -ForegroundColor Green

    if ($IncludeModel) {
        $ModelSource = Join-Path $ProjectRoot "files"
        $Required = @(
            "model.onnx",
            "config.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "special_tokens_map.json"
        )

        foreach ($name in $Required) {
            $path = Join-Path $ModelSource $name
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
                throw "Required model file is missing: $path"
            }
        }

        $modelStage = Join-Path $TempRoot "model"
        New-Item -ItemType Directory -Path $modelStage -Force | Out-Null

        foreach ($name in $Required) {
            Copy-Item -LiteralPath (Join-Path $ModelSource $name) -Destination (Join-Path $modelStage $name) -Force
        }

        $ModelZip = Join-Path $OutputDir "yad-model-multilingual-e5-small-int8.zip"
        Remove-Item -LiteralPath $ModelZip -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath "$ModelZip.sha256" -Force -ErrorAction SilentlyContinue

        Compress-Archive -Path (Join-Path $modelStage "*") -DestinationPath $ModelZip -CompressionLevel Optimal
        $ModelChecksum = Write-Checksum -File $ModelZip

        Write-Host "Created: $ModelZip" -ForegroundColor Green
        Write-Host "Created: $ModelChecksum" -ForegroundColor Green
    }

    $Manifest = [ordered]@{
        version = $VersionOutput
        created_at = (Get-Date).ToUniversalTime().ToString("o")
        platform = "windows-x64"
        binary_asset = "yad-windows-x64.zip"
        binary_checksum = "yad-windows-x64.zip.sha256"
        model_asset = $(if ($IncludeModel) { "yad-model-multilingual-e5-small-int8.zip" } else { $null })
        model_checksum = $(if ($IncludeModel) { "yad-model-multilingual-e5-small-int8.zip.sha256" } else { $null })
    }

    $ManifestPath = Join-Path $OutputDir "release-manifest.json"
    [IO.File]::WriteAllText(
        $ManifestPath,
        ($Manifest | ConvertTo-Json -Depth 5),
        (New-Object Text.UTF8Encoding($false))
    )

    Write-Host "Created: $ManifestPath" -ForegroundColor Green
}
finally {
    Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
