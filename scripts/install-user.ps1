# Installs the gated release pair for the current user so `repose` runs from any shell: copies
# target\dist\repose.exe and THIRD-PARTY-NOTICES.md (the pair scripts/stage-release.ps1 staged and a
# gate verified) to %LOCALAPPDATA%\Programs\repose and puts that folder on the user PATH when it is
# not there yet. No runtime is bundled: repose is a single executable. Nothing touches the machine
# PATH or other users. The user PATH is read and written as the raw registry value, so entries
# that hold %variables% keep them and the value keeps its REG_EXPAND_SZ type (Morticia on
# 9f74cb6; [Environment]::GetEnvironmentVariable expands them and SetEnvironmentVariable writes
# REG_SZ). A shell opened before the PATH change does not see it until reopened.
# Usage: pwsh scripts/install-user.ps1 [-Check]
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

$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
try {
    $raw = [string] $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
    $kind = if ($key.GetValueNames() -contains 'Path') { $key.GetValueKind('Path') } else { [Microsoft.Win32.RegistryValueKind]::ExpandString }
    $entries = @($raw -split ';' | Where-Object { $_ -ne '' })
    $onPath = ($entries | ForEach-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') }) -contains $target.TrimEnd('\')

    if ($Check) {
        $installed = Test-Path (Join-Path $target 'repose.exe')
        "INSTALLED $installed"
        "ON-USER-PATH $onPath (user PATH is $kind with $($entries.Count) entries)"
        if ($installed) {
            $a = (Get-FileHash (Join-Path $target 'repose.exe')).Hash
            $b = (Get-FileHash (Join-Path $dist 'repose.exe')).Hash
            "CURRENT $($a -eq $b) (installed $($a.Substring(0,12)), staged $($b.Substring(0,12)))"
        }
        exit 0
    }

    $running = Get-Process -Name repose -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.TrimEnd('\') -ieq (Join-Path $target 'repose.exe') }
    if ($running) { throw "repose is running from $target (PID $($running.Id -join ', ')); close it, then run this again" }
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    foreach ($name in $pair) {
        Copy-Item -Path (Join-Path $dist $name) -Destination (Join-Path $target $name) -Force
        "COPIED $name -> $target"
    }
    if (-not $onPath) {
        $new = if ($entries.Count -eq 0) { $target } else { (($entries + $target) -join ';') }
        # Keep the value's own type; a PATH that never existed is created as REG_EXPAND_SZ.
        $key.SetValue('Path', $new, $kind)
        "PATH-ADDED $target to the user PATH as $kind, $($entries.Count + 1) entries, existing entries byte for byte; open a new shell to use it"
    } else {
        "PATH-OK $target was already on the user PATH"
    }
} finally {
    $key.Dispose()
}
$hash = (Get-FileHash (Join-Path $target 'repose.exe')).Hash
"INSTALLED repose.exe sha256 $hash"
