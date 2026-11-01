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
Step 'cargo build --release' { cargo build --workspace --release }
# The notices travel beside the executable (quality-gate clause 9), and the release folder is
# listed so the log shows what a release would carry (clause 5): one executable, no runtimes.
$release = Join-Path $root 'target\release'
if (Test-Path (Join-Path $release 'repose.exe')) {
    Copy-Item (Join-Path $root 'THIRD-PARTY-NOTICES.md') $release -Force
    Write-Host '== release folder'
    foreach ($f in (Get-ChildItem -Path $release -File | Sort-Object Name)) {
        Write-Host ("RELEASE {0} {1}" -f $f.Name, $f.Length)
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
