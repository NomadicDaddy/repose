# Builds target\installer\repose-setup.exe (per-user-installer) from the files a gate staged in
# target\dist, with the Inno Setup 7.1.1 compiler named by REPOSE_ISCC. Run it when cutting a
# release, after a green gate on the release commit; the gate itself never needs Inno Setup, so it
# stays meaningful on a host without it, CI's runner included. The version is Cargo.toml's
# workspace version. Prints INSTALLER repose-setup.exe <bytes> <sha256> and exits 1 on any
# missing input, a compiler other than 7.1.1, or a failed compile.
# Usage: pwsh scripts/build-installer.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$iscc = $env:REPOSE_ISCC
if (-not $iscc -or -not (Test-Path $iscc)) {
    Write-Host 'FAILED: set REPOSE_ISCC to the Inno Setup 7.1.1 ISCC.exe'
    exit 1
}
$reported = (& $iscc '--version' 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $reported -notmatch '^7\.1\.1(\s|$)') {
    Write-Host "FAILED: REPOSE_ISCC reports '$reported', not Inno Setup 7.1.1"
    exit 1
}
foreach ($name in @('repose.exe', 'LICENSE', 'THIRD-PARTY-NOTICES.md', 'RUST-STD-COPYRIGHT.html')) {
    if (-not (Test-Path (Join-Path $root "target\dist\$name"))) {
        Write-Host "FAILED: missing target\dist\$name; run scripts/qc.ps1 first, which stages it"
        exit 1
    }
}
$manifest = Get-Content (Join-Path $root 'Cargo.toml') -Raw
if ($manifest -notmatch '(?m)^version = "(\d+\.\d+\.\d+)"') {
    Write-Host 'FAILED: no workspace version in Cargo.toml'
    exit 1
}
$version = $Matches[1]
$out = Join-Path $root 'target\installer'
if (Test-Path $out) { Remove-Item -Path $out -Recurse -Force }
& $iscc '/Q' "/DAppVersion=$version" (Join-Path $root 'installer\repose.iss')
if ($LASTEXITCODE -ne 0) {
    Write-Host "FAILED: ISCC exited $LASTEXITCODE"
    exit 1
}
$setup = Get-Item (Join-Path $out 'repose-setup.exe')
$hash = (Get-FileHash $setup.FullName -Algorithm SHA256).Hash.ToLower()
Write-Host ("INSTALLER {0} {1} {2} (repose {3})" -f $setup.Name, $setup.Length, $hash, $version)
exit 0
