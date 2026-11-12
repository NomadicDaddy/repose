# Refreshes an existing per-user install from the files a gate staged in target\dist (Phillip
# #31471 on #31468): scripts/qc.ps1 runs it after a green gate of a clean main. It copies the four
# files over %LOCALAPPDATA%\Programs\repose, the folder repose-setup.exe installs to, and nothing
# else: it never installs where repose is not installed, never writes the user PATH or the
# uninstall entry, and stops without copying when repose is running from that folder.
# Prints COPIED and INSTALLED lines; exits 1 with the reason otherwise.
# Usage: pwsh scripts/refresh-install.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'target\dist'
$target = Join-Path $env:LOCALAPPDATA 'Programs\repose'
$files = @('repose.exe', 'LICENSE', 'THIRD-PARTY-NOTICES.md', 'RUST-STD-COPYRIGHT.html')
foreach ($name in $files) {
    if (-not (Test-Path (Join-Path $dist $name))) { Write-Host "missing $dist\$name"; exit 1 }
}
if (-not (Test-Path (Join-Path $target 'repose.exe'))) { Write-Host "no repose installed in $target"; exit 1 }
$exe = Join-Path $target 'repose.exe'
$running = Get-Process -Name repose -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path -ieq $exe }
if ($running) { Write-Host "repose is running from $target (PID $($running.Id -join ', '))"; exit 1 }
foreach ($name in $files) {
    Copy-Item -Path (Join-Path $dist $name) -Destination (Join-Path $target $name) -Force
    Write-Host "COPIED $name -> $target"
}
Write-Host "INSTALLED repose.exe sha256 $((Get-FileHash $exe).Hash)"
exit 0
