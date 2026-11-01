# The repose gate: format, clippy, tests, release build, the 300-line file limit, then the
# measurements (reported, never failed: Phillip, 2026-11-01). Prints QC_EXIT=<code> last.
param(
    [switch] $SkipMeasure
)
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$exit = 0
function Step($name, [scriptblock] $body) {
    Write-Host "== $name"
    & $body
    if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: $name"; $script:exit = 1 }
}
Step 'cargo fmt --check' { cargo fmt --all -- --check }
Step 'cargo clippy' { cargo clippy --workspace --all-targets -- -D warnings }
Step 'cargo test' { cargo test --workspace }
# Advisories (quality-gate clause 8, Phillip Yes #31026): cargo-audit is installed per user with
# `cargo install cargo-audit`. scripts/audit.ps1 runs plain cargo audit (fetch and scan), falls
# back to the database on disk only when the fetch itself failed, says how old that database is,
# and holds the list of accepted advisory ids. Unmaintained-crate warnings are printed, not failed.
Step 'cargo audit' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'audit.ps1') }
Step 'cargo build --release' { cargo build --workspace --release }
# The pair a release carries, staged in target\dist and verified (quality-gate clauses 5 and 9):
# the executable and THIRD-PARTY-NOTICES.md, no runtimes, listed as RELEASE lines in the log.
Step 'release pair staged' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'stage-release.ps1') }
# A per-user install (scripts/install-user.ps1) is not refreshed by the gate; say when it is behind.
$installed = Join-Path $env:LOCALAPPDATA 'Programs\repose\repose.exe'
$staged = Join-Path $root 'target\dist\repose.exe'
if ((Test-Path $installed) -and (Test-Path $staged)) {
    if ((Get-FileHash $installed).Hash -ne (Get-FileHash $staged).Hash) {
        Write-Host 'INSTALL stale: the per-user copy is not this build; run scripts/install-user.ps1 to refresh it'
    } else {
        Write-Host 'INSTALL current: the per-user copy is this build'
    }
}
Write-Host '== file length (300 lines)'
$long = Get-ChildItem -Path (Join-Path $root 'crates') -Recurse -Filter *.rs |
    Where-Object { (Get-Content $_.FullName | Measure-Object -Line).Lines -gt 300 }
foreach ($f in $long) {
    Write-Host ("FAILED: {0} has {1} lines" -f $f.FullName, (Get-Content $f.FullName | Measure-Object -Line).Lines)
    $exit = 1
}
Step 'third-party notices current' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'third-party-notices.ps1') -Check }
$exe = Join-Path $root 'target\release\repose.exe'
if (Test-Path $exe) {
    $size = [math]::Round((Get-Item $exe).Length / 1MB, 2)
    Write-Host "MEASURE exe-size-mb $size (target about 25)"
    if (-not $SkipMeasure) {
        & (Join-Path $PSScriptRoot 'measure-startup.ps1') -Exe $exe
    }
}
Write-Host "QC_EXIT=$exit"
exit $exit
