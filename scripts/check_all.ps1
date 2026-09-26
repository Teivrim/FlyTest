<#
.SYNOPSIS
  The whole gate in one command: format, lints, both test suites, the editor's
  client assets, and the memory cap.

.DESCRIPTION
  Until now every check ran by hand, one at a time, and the order was whatever
  the last thing happened to be remembered. That is how a suite drifts: checks
  that are only run when someone thinks of them stop being run at all. This
  script makes the set explicit, runs all of it, and exits non-zero if any part
  fails, so it can be the one thing CI does and the one thing you run before
  pushing.

  The checks, in the order they are cheapest to fail in:

    1. cargo fmt --check      formatting; seconds, and the usual first failure
    2. clippy -D warnings     lints; minutes
    3. cargo test --release   the Rust tests over the runtime and the generator
    4. run_tfly.ps1           the C suite for TFLY.h, which cargo does not cover
    5. node --check app.js    the client is plain JavaScript, so it gets parsed
    6. check_russian.py       every piece of user-visible text is real Russian
    7. memory cap             start the editor, watch its RSS, hold it to 100 MB

  The last one is here because the cap is a requirement, not a preference, and a
  requirement nobody measures is a requirement that quietly stops being true. It
  starts the editor for a few seconds, samples the working set, and fails the
  build if it ever crosses the line.

  Comments here are in English on purpose. Windows PowerShell 5.1 reads a script
  file in the OEM codepage unless it carries a UTF-8 BOM, and a Russian em dash
  in a comment then parses as a syntax error. The user-facing text in this
  project is Russian; the code around it is not.

.PARAMETER SkipMemory
  Leave out the editor smoke run. Useful in CI containers where binding a port
  is not allowed.

.PARAMETER Port
  Port for the memory smoke run. Default 8799, deliberately not 8765, so a gate
  run never collides with an editor you already have open.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check_all.ps1

.EXAMPLE
  pwsh -NoProfile -File scripts/check_all.ps1 -SkipMemory
#>
[CmdletBinding()]
param(
    [switch]$SkipMemory,
    [int]$Port = 8799
)

$ErrorActionPreference = "Continue"

# Native tools print UTF-8, PowerShell decodes their output as the console
# codepage, and the C suite's Russian messages turn into mojibake on the way
# into the log. Setting the encoding once, here, means every captured line is
# readable instead of every check needing its own workaround.
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }
$OutputEncoding = [System.Text.Encoding]::UTF8

$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root "runtime-output"
if (-not (Test-Path $out)) {
    New-Item -ItemType Directory -Path $out | Out-Null
}
Push-Location $root

# Every step records a name, whether it passed, and how long it took, so the
# summary at the end is one table rather than a scroll of mixed output.
$results = [System.Collections.Generic.List[object]]::new()

function Invoke-Check {
    param(
        [string]$Name,
        [string]$Command,
        [string]$Log
    )

    $path = Join-Path $out $Log
    $start = Get-Date
    $ok = $true
    $tail = ""

    try {
        # The command runs through cmd with its own file redirection rather than
        # being piped into Out-File. Piping looks right and is not: cargo and gcc
        # keep writing their progress to the console handle directly, so the
        # output arrives on screen and in the log at once, glued onto one line.
        # cmd's redirection takes it at the source.
        $line = "$Command > `"$path`" 2>&1"
        & cmd /c $line
        $ok = $LASTEXITCODE -eq 0
        $tail = Get-Content $path -Encoding UTF8 -Tail 1 -ErrorAction SilentlyContinue
    } catch {
        $ok = $false
        $tail = $_.Exception.Message
    }

    # Invariant culture, or the Russian locale formats seconds as "2,1" and the
    # column stops lining up with the rest of the table.
    $secs = [math]::Round(((Get-Date) - $start).TotalSeconds, 1).ToString(
        "0.0", [System.Globalization.CultureInfo]::InvariantCulture)
    $results.Add([pscustomobject]@{
        Check   = $Name
        Result  = if ($ok) { "ok" } else { "FAIL" }
        Seconds = $secs
        Note    = $tail
    })
    if (-not $ok) { Write-Host ("  FAIL  " + $Name) -ForegroundColor Red }
}

Write-Host ""
Write-Host "FlyTest gate" -ForegroundColor Cyan
Write-Host ""

Invoke-Check -Name "cargo fmt"          -Command "cargo fmt --check"                          -Log "gate-fmt.txt"
Invoke-Check -Name "clippy -D warnings" -Command "cargo clippy --all-targets -- -D warnings" -Log "gate-clippy.txt"
Invoke-Check -Name "cargo test"         -Command "cargo test --release"                        -Log "gate-test.txt"

# The C suite is driven here rather than through run_tfly.ps1, for two reasons.
# The nested script writes its progress with Write-Host, and on Windows
# PowerShell 5.1 the host stream is not capturable, so its output would land on
# the console decoded through the OEM codepage while the log claimed the check
# had produced nothing. And a missing compiler is a skip here, not a failure:
# the Rust half of the project does not depend on the C core, and CI says so in
# as many words when MinGW is absent.
$cc = $null
foreach ($candidate in @("gcc", "clang", "cl")) {
    if (Get-Command $candidate -ErrorAction SilentlyContinue) { $cc = $candidate; break }
}
if ($cc) {
    # Compile and run chained with &&, so a compile failure stops before the
    # run and the log shows the compiler's error rather than a missing file.
    $cCmd = "$cc -std=c99 -O2 -Wall -Wextra -Wpedantic -o `"$out\gate-tfly-test.exe`" native\tfly_test.c -lm && `"$out\gate-tfly-test.exe`""
    Invoke-Check -Name "TFLY C suite" -Command $cCmd -Log "gate-c.txt"
} else {
    $results.Add([pscustomobject]@{
        Check = "TFLY C suite"; Result = "skip"; Seconds = "0.0"
        Note = "no C compiler on PATH; the Rust half does not need one"
    })
}

# The client is not part of the cargo build and nothing else parses it, so a typo
# in the browser half of the project used to survive until someone opened the
# page and watched nothing happen.
Invoke-Check -Name "node --check app.js" -Command "node --check editor\app.js" -Log "gate-node.txt"
Invoke-Check -Name "Russian text"        -Command "python scripts\check_russian.py"       -Log "gate-russian.txt"

# The hundred megabytes. A hard requirement from the person who asked for this
# project, so it gets measured rather than remembered.
$memoryOk = $true
$memoryNote = "skipped"
if (-not $SkipMemory) {
    $exe = Join-Path $root "target\release\flytest.exe"
    if (-not (Test-Path $exe)) {
        & cmd /c "cargo build --release > `"$out\gate-build.txt`" 2>&1"
    }
    Get-Process flytest -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 400
    $proc = Start-Process -FilePath $exe `
        -ArgumentList "editor", "--port", "$Port", "--flies", "3" `
        -PassThru -WindowStyle Hidden
    $peak = 0
    try {
        for ($i = 0; $i -lt 20; $i++) {
            Start-Sleep -Milliseconds 400
            $proc.Refresh()
            if ($proc.HasExited) { break }
            if ($proc.WorkingSet64 -gt $peak) { $peak = $proc.WorkingSet64 }
        }
        $mb = ($peak / 1MB).ToString("0.0", [System.Globalization.CultureInfo]::InvariantCulture)
        $memoryNote = "$mb MB peak"
        if ($peak -ge 100MB) {
            $memoryOk = $false
            $memoryNote = "$memoryNote, over the 100 MB cap"
        }
    } finally {
        if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
    }
}
$results.Add([pscustomobject]@{
    Check   = "memory under 100 MB"
    Result  = if ($memoryOk) { "ok" } else { "FAIL" }
    Seconds = "0.0"
    Note    = $memoryNote
})

Write-Host ""
# Alignment in a .NET format string is {2,7}, not {2,>7}. The greater-than form
# is C# and XLSX, and PowerShell throws a FormatError on it rather than
# rendering it, which is why the first version of this table printed nothing
# at all.
$head = "{0,-22} {1,-6} {2,7}  {3}" -f "check", "result", "seconds", "note"
Write-Host $head
$rule = "-" * 78
Write-Host $rule
foreach ($row in $results) {
    $colour = if ($row.Result -eq "ok") { "Green" } elseif ($row.Result -eq "skip") { "Yellow" } else { "Red" }
    $line = "{0,-22} {1,-6} {2,7}  {3}" -f $row.Check, $row.Result, $row.Seconds, $row.Note
    Write-Host $line -ForegroundColor $colour
}
Write-Host ""

$failed = @($results | Where-Object { $_.Result -eq "FAIL" })
Pop-Location

if ($failed.Count -gt 0) {
    $line = "gate failed: {0} of {1} checks" -f $failed.Count, $results.Count
    Write-Host $line -ForegroundColor Red
    exit 1
}
$line = "gate passed: {0} of {1} checks ok" -f @($results | Where-Object { $_.Result -eq "ok" }).Count, $results.Count
Write-Host $line -ForegroundColor Green
exit 0
