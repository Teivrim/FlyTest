[CmdletBinding()]
param(
    [switch]$WithBlender,
    [int]$Ticks = 600,
    [string]$Blender = 'blender'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$outputDir = Join-Path $root 'runtime-output'
New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
$events = Join-Path $outputDir 'events.jsonl'

Push-Location $root
try {
    cargo run --release -- circus --ticks $Ticks --output $events
    if ($WithBlender) {
        $scene = Join-Path $outputDir 'flytest.blend'
        & $Blender --background --python (Join-Path $root 'blender/flytest_bridge.py') -- --events $events --output $scene --fps 60
        if ($LASTEXITCODE -ne 0) { throw "Blender bridge failed with exit code $LASTEXITCODE" }
        Write-Host "Scene written to $scene"
    } else {
        Write-Host "Events written to $events"
        Write-Host "Run again with -WithBlender after installing/configuring Blender."
    }
} finally {
    Pop-Location
}
