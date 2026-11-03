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
# What a release carries, staged in target\dist and verified (quality-gate clauses 5 and 9): the
# executable, LICENSE, THIRD-PARTY-NOTICES.md and the toolchain's RUST-STD-COPYRIGHT.html, no
# runtimes, listed as RELEASE lines in the log.
Step 'release pair staged' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'stage-release.ps1') }
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
# A per-user install (scripts/install-user.ps1) is refreshed by a green gate (Phillip #31471 on
# #31468): when every step passed and a per-user copy exists, the staged pair is copied over it,
# unless that repose is running, in which case the gate says so and the next green gate does it.
# A failed gate leaves the copy alone and says whether it is behind, judging every staged file
# (Roger on ba3a580). Nothing here changes the exit code.
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\repose'
if (Test-Path (Join-Path $installDir 'repose.exe')) {
    $behind = @()
    foreach ($name in @('repose.exe', 'LICENSE', 'THIRD-PARTY-NOTICES.md', 'RUST-STD-COPYRIGHT.html')) {
        $here = Join-Path $installDir $name
        $there = Join-Path $root "target\dist\$name"
        if (-not (Test-Path $here) -or -not (Test-Path $there) -or (Get-FileHash $here).Hash -ne (Get-FileHash $there).Hash) { $behind += $name }
    }
    if ($behind.Count -eq 0) {
        Write-Host 'INSTALL current: the per-user copy is this build, every staged file'
    } elseif ($exit -ne 0) {
        Write-Host "INSTALL kept: the gate failed, so the per-user copy stays as it is, behind in $($behind -join ', ')"
    } else {
        # Only a build of a commit on main reaches the per-user copy: a dirty tree or another branch
        # would install bytes that match no commit the log names (Morticia M-1 on 2f884b9).
        # Each Git answer counts only when Git succeeded: a failed status with empty output must
        # keep the copy, not pass as clean (Roger on 2c7e596). symbolic-ref -q exits 1 when HEAD
        # is detached, which is a plain not-main answer, not a failure. Only stdout is the answer:
        # on this host git status writes two global-ignore permission warnings to stderr with
        # exit 0, and folded in they read as two dirty paths (Roger on 29bcb6b).
        $dirty = @(git status --porcelain 2>$null | ForEach-Object { "$_" } | Where-Object { $_ -ne '' })
        $statusOk = ($LASTEXITCODE -eq 0)
        $head = "$(git symbolic-ref -q HEAD 2>$null)"
        if (-not $head) { $head = 'detached' }
        $commit = "$(git rev-parse --short HEAD 2>$null)"
        $commitOk = ($LASTEXITCODE -eq 0)
        if (-not $statusOk -or -not $commitOk) {
            Write-Host "INSTALL kept: git could not report the tree state (status exit ok: $statusOk, rev-parse ok: $commitOk), so the copy is left alone"
        } elseif ($dirty.Count -gt 0) {
            Write-Host "INSTALL kept: the tree has $($dirty.Count) uncommitted path(s), so this build matches no commit; commit first"
        } elseif ($head -ne 'refs/heads/main') {
            Write-Host "INSTALL kept: HEAD is $head, not refs/heads/main"
        } else {
            # Copy only: the gate never writes the user PATH, so a folder removed from it stays
            # removed and the script says so (Morticia L-1 on 2f884b9); every PATH line is echoed.
            $lines = @(& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'install-user.ps1') -CopyOnly 2>&1 | ForEach-Object { "$_" })
            if ($LASTEXITCODE -eq 0) {
                Write-Host "INSTALL refreshed: the per-user copy is now main $commit ($($behind -join ', ') replaced)"
                $lines | Where-Object { $_ -like 'INSTALLED *' -or $_ -like 'PATH-*' } | ForEach-Object { Write-Host $_ }
            } else {
                $why = $lines | Where-Object { $_ -match 'repose is running|missing ' } | Select-Object -First 1
                if (-not $why) { $why = "install-user.ps1 exited $LASTEXITCODE" }
                Write-Host "INSTALL skipped: $why; the next green gate refreshes it"
            }
        }
    }
}
Write-Host "QC_EXIT=$exit"
exit $exit
