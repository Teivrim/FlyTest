<#
.SYNOPSIS
  Build and run the TFLY.h C tests and demo.

.DESCRIPTION
  TFLY.h is a single-header C library, so it is verified directly with a C
  compiler rather than through cargo. The Rust bindings in src/tfly.rs are
  then exercised separately by `cargo test tfly`.

.PARAMETER Compiler
  C compiler to use. Defaults to gcc on PATH, or cl if that is all there is.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run_tfly.ps1
#>
[CmdletBinding()]
param(
    [string]$Compiler = "",
    [switch]$SkipDemo
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root "runtime-output"

if (-not (Test-Path $out)) {
    New-Item -ItemType Directory -Path $out | Out-Null
}

function Resolve-Compiler {
    param([string]$Preferred)
    if ($Preferred) {
        if (Get-Command $Preferred -ErrorAction SilentlyContinue) { return $Preferred }
        throw "compiler not found: $Preferred"
    }
    foreach ($candidate in @("gcc", "clang")) {
        if (Get-Command $candidate -ErrorAction SilentlyContinue) { return $candidate }
    }
    throw "no C compiler found. Install gcc/clang or pass -Compiler."
}

$cc = Resolve-Compiler -Preferred $Compiler
Write-Host "TFLY.h self test"
Write-Host "compiler: $cc"
Write-Host ""

$targets = @(
    @{ Name = "tfly_test"; Source = "native/tfly_test.c"; Run = $true },
    @{ Name = "tfly_demo"; Source = "native/tfly_demo.c"; Run = -not $SkipDemo }
)

$failed = $false
foreach ($target in $targets) {
    $exe = Join-Path $out "$($target.Name).exe"
    if ($target.Name -eq "tfly_demo" -and $SkipDemo) { continue }

    $args = @("-std=c99", "-O2", "-Wall", "-Wextra", "-Wpedantic", "-o", $exe, $target.Source, "-lm")
    & $cc @args
    if ($LASTEXITCODE -ne 0) {
        Write-Host "BUILD FAILED: $($target.Name)" -ForegroundColor Red
        $failed = $true
        continue
    }

    if ($target.Run) {
        & $exe
        if ($LASTEXITCODE -ne 0) {
            Write-Host "TEST FAILED: $($target.Name)" -ForegroundColor Red
            $failed = $true
        }
    }
}

if ($failed) {
    Write-Host ""
    Write-Host "TFLY checks failed" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "TFLY.h checks passed" -ForegroundColor Green
Write-Host "Rust bindings: cargo test tfly"
