# The gate's advisory check (quality-gate clause 8, Phillip Yes #31026). Plain `cargo audit`
# refreshes the RustSec database and scans; cargo-audit 0.22 has no fetch subcommand (Morticia
# M-2 on 4f22cd3). Only when that run failed on the fetch itself, not on a finding, the scan
# reruns against the database on disk with --no-fetch, and the log says how old that database
# is, so an offline gate is never mistaken for a fresh one. Exit code: cargo audit's.
#
# An accepted advisory goes in this list as its RUSTSEC id with the room message that accepted
# it, so nothing is ignored silently; none so far. The list lives here rather than being passed
# from qc.ps1, because an array through `pwsh -File` arrives as one value or none (Morticia M-3
# on 1d7c871).
$ErrorActionPreference = 'Continue'
$acceptedAdvisories = @()
$ignoreArgs = @($acceptedAdvisories | ForEach-Object { '--ignore'; $_ })
$output = & cargo audit @ignoreArgs 2>&1 | ForEach-Object { "$_" }
$code = $LASTEXITCODE
$output | ForEach-Object { Write-Host $_ }
$fetchFailed = ($code -ne 0) -and (($output -join "`n") -match "couldn't fetch|Couldn't fetch|failed to fetch|fetch.*advisory|unable to update|error.*git")
$dbDir = Join-Path $HOME '.cargo\advisory-db'
if (Test-Path (Join-Path $dbDir '.git')) {
    $dbDate = (git -C $dbDir log -1 --format=%cI 2>$null)
    if ($dbDate) {
        $age = [math]::Round(((Get-Date) - [datetime]$dbDate).TotalDays, 1)
        Write-Host "ADVISORY-DB last commit $dbDate ($age days old)"
        if ($age -gt 7) { Write-Host "WARNING: advisory database is $age days old" }
    }
}
if (-not $fetchFailed) { exit $code }
Write-Host 'NOTE: the advisory database could not be refreshed (no network?); scanning the copy on disk'
& cargo audit --no-fetch @ignoreArgs
exit $LASTEXITCODE
