# The repose gate: format, clippy, tests, advisory audit, release build, release staging, the
# 300-line file limit and the notices check; then the measurements (reported, never failed:
# Phillip, 2026-11-01) and, after a green run on a clean main, the per-user copy refresh, which
# never changes the exit code. Prints QC_EXIT=<code> last.
#
# Steps that passed before with exactly the same inputs are not rerun (quality-gate-cache; see
# scripts/gate-cache.ps1): format, clippy, tests, the release build, the 300-line check, the
# notices check and the startup and idle measurements. A hit prints CACHED with the time of the
# original pass. The advisory audit and release staging always run, and a failure is never cached.
# -NoCache runs every step regardless, and still records passes.
param(
    [switch] $SkipMeasure,
    [switch] $NoCache
)
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$GateRoot = $root
. (Join-Path $PSScriptRoot 'gate-cache.ps1')
$exit = 0
function Step($name, [scriptblock] $body) {
    Write-Host "== $name"
    $global:LASTEXITCODE = 0
    # As in Invoke-GateStep: a command that cannot start sets no exit code, so the try catches it.
    $started = $true
    try { & $body } catch { $started = $false; Write-Host "ERROR: $name could not run: $($_.Exception.Message)" }
    if ($LASTEXITCODE -ne 0 -or -not $started) { Write-Host "FAILED: $name"; $script:exit = 1 }
}
# What every Rust step's result depends on: the sources, assets, manifests, lockfile and tool
# configuration; the gate scripts themselves; and the toolchain and environment cargo reads.
$gateScripts = @('scripts/qc.ps1', 'scripts/gate-cache.ps1')
$rustInputs = @('crates', 'Cargo.toml', 'Cargo.lock', 'clippy.toml', 'rustfmt.toml', 'rust-toolchain.toml') + $gateScripts
$rustFacts = @(
    "rustc $(rustc -vV 2>&1)", "cargo $(cargo -V 2>&1)", "rustfmt $(cargo fmt --version 2>&1)",
    "clippy $(cargo clippy --version 2>&1)"
)
foreach ($var in @('RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET', 'CARGO_TARGET_DIR')) {
    $rustFacts += "env $var=$([Environment]::GetEnvironmentVariable($var))"
}
function CachedStep($name, [string[]] $paths, [string[]] $facts, [scriptblock] $body, [string] $artifact = '') {
    if (-not (Invoke-GateStep -Name $name -Paths $paths -Facts $facts -Body $body -Artifact $artifact -NoCache:$NoCache)) { $script:exit = 1 }
}
Step 'gate cache self-test' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'gate-cache-test.ps1') }
# The delivered leak guard is tested on every gate, in every clone, whether or not the hooks are
# installed (audit-secret-handling-retention-1794018053), by the self-test smokestack delivers with
# it (scripts/smokestack/check-leak-guard.sh, its cases and setup; it builds scratch repositories in
# a temp folder and always names a fixture pattern file). It runs under Git's own bash, the one the
# hooks use, found beside git: a bare `bash` can be WSL's. Never cached: it is quick, and its input
# includes the machine's git.
$gitBash = $null
$gitDir = Split-Path -Parent "$((Get-Command git -ErrorAction SilentlyContinue).Source)"
while ($gitDir -and -not $gitBash) {
    $gitBash = @('usr\bin\bash.exe', 'bin\bash.exe') | ForEach-Object { Join-Path $gitDir $_ } |
        Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    $gitDir = Split-Path -Parent $gitDir
}
Step 'leak guard self-test' {
    if (-not $gitBash) { throw "no bash.exe beside git" }
    & $gitBash (Join-Path $PSScriptRoot 'smokestack\check-leak-guard.sh')
}
CachedStep 'cargo fmt --check' $rustInputs $rustFacts { cargo fmt --all -- --check }
CachedStep 'cargo clippy' $rustInputs $rustFacts { cargo clippy --workspace --all-targets -- -D warnings }
# The tests drive the installed Git, so its version is part of their inputs. So is the machine's
# git configuration: the tests' own fixture git calls set it aside (crates/core/tests/support),
# but the code under test spawns git as the app does, with the developer's global and system
# config, so a change to either is a miss. Only a hash of each enters the key, never the text.
$gitFacts = @("git $(git --version 2>&1)")
foreach ($scope in @('global', 'system')) {
    $config = [System.Text.Encoding]::UTF8.GetBytes((@(git config "--$scope" --includes --list 2>$null) -join "`n"))
    $gitFacts += "git $scope config $([System.Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($config)))"
}
# --no-fail-fast: every test binary runs even after one fails, so a failing gate names every
# failure at once, not only the first binary's (Morticia 5Q-1 on 5f7927c). The step still fails.
CachedStep 'cargo test' $rustInputs ($rustFacts + $gitFacts) { cargo test --workspace --no-fail-fast }
# Advisories (quality-gate clause 8, Phillip Yes #31026): cargo-audit is installed per user with
# `cargo install cargo-audit`. scripts/audit.ps1 runs plain cargo audit (fetch and scan), falls
# back to the database on disk only when the fetch itself failed, says how old that database is,
# and holds the list of accepted advisory ids. Unmaintained-crate warnings are printed, not failed.
Step 'cargo audit' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'audit.ps1') }
# A hit also needs target\release\repose.exe to be the executable that pass produced.
# crates/app/build.rs stamps the commit into the executable (HEAD, plus "+dirty" when tracked
# files differ), so the build's key carries both as build.rs works them out: committing the same
# bytes is a miss, and the exe the gate installs names the commit it came from (Morticia M-1 on
# c9cc1be). Only stdout counts as Git's answer.
$stampHead = "$(git rev-parse HEAD 2>$null)"
$stampDirty = @(git status --porcelain --untracked-files=no 2>$null | Where-Object { "$_" -ne '' }).Count -gt 0
$buildFacts = $rustFacts + "build stamp HEAD $stampHead dirty $stampDirty"
CachedStep 'cargo build --release' $rustInputs $buildFacts { cargo build --workspace --release } 'target/release/repose.exe'
# What a release carries, staged in target\dist and verified (quality-gate clauses 5 and 9): the
# executable, LICENSE, THIRD-PARTY-NOTICES.md and the toolchain's RUST-STD-COPYRIGHT.html, no
# runtimes, listed as RELEASE lines in the log.
Step 'release pair staged' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'stage-release.ps1') }
# Physical lines, blank ones included, as the fleet's check:max-lines counts them; Measure-Object
# -Line skipped blanks and let files of 302-308 lines through (Morticia L-1 on 8eebf0e). @()
# keeps a one-line file a one-element array rather than a string.
CachedStep 'file length (300 physical lines)' (@('crates') + $gateScripts) @() {
    $long = @(Get-ChildItem -Path (Join-Path $root 'crates') -Recurse -Filter *.rs |
        Where-Object { @(Get-Content $_.FullName).Count -gt 300 })
    foreach ($f in $long) { Write-Host ("FAILED: {0} has {1} lines" -f $f.FullName, @(Get-Content $f.FullName).Count) }
    $global:LASTEXITCODE = [int] ($long.Count -gt 0)
}
# The notices depend on the manifests, lockfile, the two licence texts the generator copies in
# whole (LICENSE and the vendored PowerShell grammar's LICENSE.txt), the licence folder, the
# notices file and script, and the toolchain, whose standard-library notice the file records by
# hash.
$sysroot = "$(rustc --print sysroot 2>$null)".Trim()
$stdNotice = Join-Path $sysroot 'share\doc\rust\COPYRIGHT-library.html'
$stdHash = if (Test-Path -LiteralPath $stdNotice) { (Get-FileHash -LiteralPath $stdNotice -Algorithm SHA256).Hash } else { 'missing' }
$manifests = @(Get-ChildItem -Path (Join-Path $root 'crates') -Recurse -Filter Cargo.toml | ForEach-Object { $_.FullName.Substring($root.Length + 1) })
$noticeInputs = @('Cargo.toml', 'Cargo.lock', 'LICENSE', 'licenses', 'crates/app/syntaxes/PowerShell/LICENSE.txt', 'THIRD-PARTY-NOTICES.md', 'scripts/third-party-notices.ps1') + $manifests + $gateScripts
CachedStep 'third-party notices current' $noticeInputs ($rustFacts + "rust-std notice $stdHash") {
    pwsh -NoProfile -File (Join-Path $PSScriptRoot 'third-party-notices.ps1') -Check
}
$exe = Join-Path $root 'target\release\repose.exe'
if (Test-Path $exe) {
    $size = [math]::Round((Get-Item $exe).Length / 1MB, 2)
    Write-Host "MEASURE exe-size-mb $size (target about 25)"
    if (-not $SkipMeasure) {
        # The startup and idle samples are reused only for the same executable bytes, measurement
        # script and conditions, and only when both were measured; a reused line says when it was
        # measured, so it never reads as a fresh sample.
        $measureName = 'startup and idle measurements'
        $measureKey = Get-GateInputKey @('target/release/repose.exe', 'scripts/measure-startup.ps1') @('qc runs measure-startup.ps1 -Exe target/release/repose.exe with its default IdleSeconds')
        $hit = if ($NoCache) { $null } else { Get-GatePass $measureName $measureKey }
        if ($hit) {
            foreach ($line in $hit.lines) { Write-Host "$line [reused: measured at $($hit.at) on this same executable, script and conditions]" }
        } else {
            $measured = @(& (Join-Path $PSScriptRoot 'measure-startup.ps1') -Exe $exe 6>&1 | ForEach-Object { "$_" })
            $measured | ForEach-Object { Write-Host $_ }
            $lines = @($measured | Where-Object { $_ -like 'MEASURE *' })
            if ($lines.Count -eq 2 -and -not ($lines | Where-Object { $_ -match ' unavailable' })) {
                Save-GatePass $measureName $measureKey '' $lines
            } else {
                Clear-GatePass $measureName
            }
        }
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
