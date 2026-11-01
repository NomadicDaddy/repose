# Stages what a release would carry, and nothing else, in target\dist: the executable and
# THIRD-PARTY-NOTICES.md beside it. Each copy is verified by hash against its source, and the
# folder is listed as RELEASE <name> <bytes> lines, so the gate log shows the pair that travels
# rather than the build folder with its .rlib, .pdb and lock files (Roger 5Q-1 on 1a4888a).
# Exits 1 on any missing or mismatched file. Usage: pwsh scripts/stage-release.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'target\dist'
$pairs = @(
    @{ from = (Join-Path $root 'target\release\repose.exe'); name = 'repose.exe' },
    @{ from = (Join-Path $root 'THIRD-PARTY-NOTICES.md'); name = 'THIRD-PARTY-NOTICES.md' }
)
foreach ($p in $pairs) {
    if (-not (Test-Path $p.from)) { Write-Host "FAILED: missing $($p.from)"; exit 1 }
}
if (Test-Path $dist) { Remove-Item -Path $dist -Recurse -Force }
New-Item -ItemType Directory -Path $dist | Out-Null
foreach ($p in $pairs) {
    $to = Join-Path $dist $p.name
    Copy-Item -Path $p.from -Destination $to
    $a = (Get-FileHash -Path $p.from -Algorithm SHA256).Hash
    $b = (Get-FileHash -Path $to -Algorithm SHA256).Hash
    if ($a -cne $b) { Write-Host "FAILED: $($p.name) differs from its source after the copy"; exit 1 }
}
$files = @(Get-ChildItem -Path $dist -File | Sort-Object Name)
if ($files.Count -ne $pairs.Count) { Write-Host "FAILED: target\dist holds $($files.Count) files, expected $($pairs.Count)"; exit 1 }
Write-Host '== release pair (target\dist)'
foreach ($f in $files) { Write-Host ("RELEASE {0} {1}" -f $f.Name, $f.Length) }
exit 0
