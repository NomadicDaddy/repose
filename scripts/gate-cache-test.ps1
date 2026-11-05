# Self-test of the gate's step cache (quality-gate-cache clause 6), run by qc.ps1 on every gate.
# Works in a fresh folder under the temp directory and removes only that folder. Prints one
# PASS/FAIL line per check and exits 1 when any check fails.
$ErrorActionPreference = 'Stop'
$GateRoot = Join-Path ([System.IO.Path]::GetTempPath()) "repose-gate-cache-$([guid]::NewGuid().ToString('N'))"
[System.IO.Directory]::CreateDirectory($GateRoot) | Out-Null
. (Join-Path $PSScriptRoot 'gate-cache.ps1')
$failures = 0
function Check([string] $what, [bool] $ok) {
    if ($ok) { Write-Host "PASS $what" } else { Write-Host "FAIL $what"; $script:failures++ }
}
# Runs a step whose body counts its runs and exits with $code; returns whether the step passed.
$script:runs = 0
function Run([int] $code, [string[]] $facts = @('tool 1'), [string] $artifact = '', [switch] $NoCache) {
    Invoke-GateStep -Name 'demo step' -Paths @('src', 'Cargo.lock') -Facts $facts -Artifact $artifact -NoCache:$NoCache -Body {
        $script:runs++
        $global:LASTEXITCODE = $code
    } 6>$null
}
try {
    New-Item -ItemType Directory -Path (Join-Path $GateRoot 'src') | Out-Null
    Set-Content -LiteralPath (Join-Path $GateRoot 'src\a.rs') -Value 'fn a() {}'
    Set-Content -LiteralPath (Join-Path $GateRoot 'Cargo.lock') -Value 'lock 1'

    $ok = Run 0; Check 'a first pass runs the step' ($ok -and $runs -eq 1)
    $ok = Run 0; Check 'the same inputs are a hit: the step does not run again' ($ok -and $runs -eq 1)
    $ok = Run 0 -NoCache; Check '-NoCache runs the step anyway' ($ok -and $runs -eq 2)

    Set-Content -LiteralPath (Join-Path $GateRoot 'src\a.rs') -Value 'fn a() { 1; }'
    $ok = Run 0; Check 'a changed source file is a miss' ($ok -and $runs -eq 3)
    Set-Content -LiteralPath (Join-Path $GateRoot 'src\b.rs') -Value 'fn b() {}'
    $ok = Run 0; Check 'an added source file is a miss' ($ok -and $runs -eq 4)
    Remove-Item -LiteralPath (Join-Path $GateRoot 'src\b.rs')
    $ok = Run 0; Check 'a removed source file is a miss' ($ok -and $runs -eq 5)
    $ok = Run 0 @('tool 2'); Check 'a changed tool version or setting is a miss' ($ok -and $runs -eq 6)
    Set-Content -LiteralPath (Join-Path $GateRoot 'Cargo.lock') -Value 'lock 2'
    $ok = Run 0 @('tool 2'); Check 'a changed lockfile or script input is a miss' ($ok -and $runs -eq 7)

    # New inputs, so the step runs (the old pass does not cover them) and fails.
    Set-Content -LiteralPath (Join-Path $GateRoot 'src\a.rs') -Value 'fn a() { broken'
    $ok = Run 1 @('tool 2'); Check 'a failing step reports failure' (-not $ok -and $runs -eq 8)
    Check 'a failure leaves no record' (-not (Test-Path -LiteralPath (Get-GateCacheFile 'demo step')))
    $ok = Run 1 @('tool 2'); Check 'a failed step reruns on the next gate' (-not $ok -and $runs -eq 9)
    $ok = Run 0 @('tool 2'); Check 'once it passes it is recorded' ($ok -and $runs -eq 10)
    $ok = Run 0 @('tool 2'); Check '...and then hits' ($ok -and $runs -eq 10)

    Set-Content -LiteralPath (Get-GateCacheFile 'demo step') -Value '{ not json'
    $ok = Run 0 @('tool 2'); Check 'an unparseable record is a miss' ($ok -and $runs -eq 11)
    $record = Get-Content -LiteralPath (Get-GateCacheFile 'demo step') -Raw | ConvertFrom-Json
    $record.result = 'fail'
    $record | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Get-GateCacheFile 'demo step')
    $ok = Run 0 @('tool 2'); Check 'a record that does not say pass is a miss' ($ok -and $runs -eq 12)

    Set-Content -LiteralPath (Join-Path $GateRoot 'out.exe') -Value 'binary 1'
    $ok = Run 0 @('tool 2') 'out.exe'; Check 'a step with an artifact records it' ($ok -and $runs -eq 13)
    $ok = Run 0 @('tool 2') 'out.exe'; Check 'the same artifact is a hit' ($ok -and $runs -eq 13)
    Set-Content -LiteralPath (Join-Path $GateRoot 'out.exe') -Value 'binary 2'
    $ok = Run 0 @('tool 2') 'out.exe'; Check 'a changed artifact is a miss' ($ok -and $runs -eq 14)
    Remove-Item -LiteralPath (Join-Path $GateRoot 'out.exe')
    $ok = Run 0 @('tool 2') 'out.exe'; Check 'a missing artifact is a miss, and a pass that produced none fails' (-not $ok -and $runs -eq 15)
    Check '...and leaves no record' (-not (Test-Path -LiteralPath (Get-GateCacheFile 'demo step')))
} finally {
    Remove-Item -LiteralPath $GateRoot -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Host "gate cache self-test: $failures failure(s)"
exit [int] ($failures -gt 0)
