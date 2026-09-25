[CmdletBinding()]
param(
    [string]$DataDir = (Join-Path $PSScriptRoot '..\data\flywire\fafb-v783'),
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$manifestPath = Join-Path $DataDir 'manifest.json'
if (-not (Test-Path -LiteralPath $manifestPath)) {
    throw "Manifest not found: $manifestPath"
}

New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json

foreach ($file in $manifest.files) {
    $target = Join-Path $DataDir $file.name
    if ((Test-Path -LiteralPath $target) -and -not $Force) {
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $target).Hash.ToLowerInvariant()
        if ($hash -eq $file.sha256.ToLowerInvariant()) {
            Write-Host "OK       $($file.name)"
            continue
        }
        Write-Warning "Hash mismatch; downloading $($file.name) again"
    }

    $partial = "$target.part"
    Write-Host "Download $($file.name) ..."
    & curl.exe --fail --location --silent --show-error --retry 3 --output $partial $file.url
    if ($LASTEXITCODE -ne 0) {
        throw "Download failed: $($file.url)"
    }

    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $partial).Hash.ToLowerInvariant()
    if ($hash -ne $file.sha256.ToLowerInvariant()) {
        Remove-Item -Force -ErrorAction SilentlyContinue $partial
        throw "SHA-256 mismatch: $($file.name)"
    }
    Move-Item -Force -LiteralPath $partial -Destination $target
    Write-Host "OK       $($file.name)"
}

Write-Host "FlyWire data ready in $DataDir"
