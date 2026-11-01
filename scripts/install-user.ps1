# Installs the gated release pair for the current user so `repose` runs from any shell: copies
# target\dist\repose.exe and THIRD-PARTY-NOTICES.md (as scripts/stage-release.ps1 staged them, the
# pair a gate verified) to %LOCALAPPDATA%\Programs\repose and puts that folder on the user PATH
# when it is not there yet. No runtime is bundled: repose is a single executable. Nothing touches
# the machine PATH, other users or the registry beyond HKCU\Environment. A shell opened before
# the PATH change does not see it until reopened. Usage: pwsh scripts/install-user.ps1 [-Check]
param(
    [switch] $Check
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'target\dist'
$target = Join-Path $env:LOCALAPPDATA 'Programs\repose'
$pair = @('repose.exe', 'THIRD-PARTY-NOTICES.md')
foreach ($name in $pair) {
    if (-not (Test-Path (Join-Path $dist $name))) { throw "missing $dist\$name; run scripts/qc.ps1 first, which stages the gated pair" }
}
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$onPath = ($userPath -split ';' | Where-Object { $_ } | ForEach-Object { $_.TrimEnd('\') }) -contains $target.TrimEnd('\')
if ($Check) {
    $installed = Test-Path (Join-Path $target 'repose.exe')
    "INSTALLED $installed"
    "ON-USER-PATH $onPath"
    if ($installed) {
        $a = (Get-FileHash (Join-Path $target 'repose.exe')).Hash
        $b = (Get-FileHash (Join-Path $dist 'repose.exe')).Hash
        "CURRENT $($a -eq $b) (installed $($a.Substring(0,12)), staged $($b.Substring(0,12)))"
    }
    exit 0
}
New-Item -ItemType Directory -Force -Path $target | Out-Null
foreach ($name in $pair) {
    Copy-Item -Path (Join-Path $dist $name) -Destination (Join-Path $target $name) -Force
    "COPIED $name -> $target"
}
if (-not $onPath) {
    $new = if ([string]::IsNullOrEmpty($userPath)) { $target } else { "$($userPath.TrimEnd(';'));$target" }
    [Environment]::SetEnvironmentVariable('Path', $new, 'User')
    "PATH-ADDED $target (user PATH; open a new shell to use it)"
} else {
    "PATH-OK $target was already on the user PATH"
}
$hash = (Get-FileHash (Join-Path $target 'repose.exe')).Hash
"INSTALLED repose.exe sha256 $hash"
