# Installs the gated release pair for the current user so `repose` runs from any shell: copies
# target\dist\repose.exe with its LICENSE, THIRD-PARTY-NOTICES.md and RUST-STD-COPYRIGHT.html (the
# files scripts/stage-release.ps1 staged and a gate verified) to %LOCALAPPDATA%\Programs\repose and puts that folder on the user PATH when it is
# not there yet. No runtime is bundled: repose is a single executable. Nothing touches the machine
# PATH or other users. The user PATH is read and written as the raw registry value, so entries
# that hold %variables% keep them and the value keeps its REG_EXPAND_SZ type (Morticia on
# 9f74cb6; [Environment]::GetEnvironmentVariable expands them and SetEnvironmentVariable writes
# REG_SZ). A shell opened before the PATH change does not see it until reopened.
# Usage: pwsh scripts/install-user.ps1 [-Check] [-CopyOnly]
# -CopyOnly copies the pair and leaves the user PATH alone, reporting when the folder is not on
# it; the gate's refresh uses it, so a folder taken off the PATH on purpose stays off (Morticia
# L-1 on 2f884b9).
param(
    [switch] $Check,
    [switch] $CopyOnly
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'target\dist'
$target = Join-Path $env:LOCALAPPDATA 'Programs\repose'
$pair = @('repose.exe', 'LICENSE', 'THIRD-PARTY-NOTICES.md', 'RUST-STD-COPYRIGHT.html')
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
            # Every staged file, so a stale or missing notices file is never called current
            # (Roger on 9f74cb6).
            $current = $true
            foreach ($name in $pair) {
                $here = Join-Path $target $name
                if (-not (Test-Path $here)) { "MISSING $name"; $current = $false; continue }
                $a = (Get-FileHash $here).Hash
                $b = (Get-FileHash (Join-Path $dist $name)).Hash
                "$name installed $($a.Substring(0,12)) staged $($b.Substring(0,12)) $(if ($a -eq $b) { 'same' } else { 'DIFFERENT' })"
                if ($a -ne $b) { $current = $false }
            }
            "CURRENT $current"
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
    if (-not $onPath -and $CopyOnly) {
        "PATH-MISSING $target is not on the user PATH and -CopyOnly leaves it so; run scripts/install-user.ps1 without -CopyOnly to add it"
    } elseif (-not $onPath) {
        # Append to the raw value as it is, never split and rejoined, so empty segments and
        # every byte of the existing entries survive (Roger on ba3a580). The value keeps its own
        # type; a PATH that never existed is created as REG_EXPAND_SZ.
        $new = if ($raw -eq '') { $target } elseif ($raw.EndsWith(';')) { "$raw$target" } else { "$raw;$target" }
        $key.SetValue('Path', $new, $kind)
        "PATH-ADDED $target to the user PATH as $kind; the existing value is kept byte for byte with the folder appended"
        # Registry writes do not tell running programs; Explorer learns of the change from this
        # broadcast, so a shell started from the Start menu or the taskbar afterwards sees the new
        # PATH. Shells already open do not (Morticia on ba3a580).
        Add-Type -Namespace Win -Name Env -MemberDefinition '[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)] public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);'
        $result = [UIntPtr]::Zero
        $sent = [Win.Env]::SendMessageTimeout([IntPtr] 0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref] $result)
        if ($sent -ne [IntPtr]::Zero) {
            "PATH-BROADCAST WM_SETTINGCHANGE sent; shells started from Explorer from now on see it, shells already open do not"
        } else {
            "PATH-BROADCAST failed (code $([System.Runtime.InteropServices.Marshal]::GetLastWin32Error())); sign out and in, or open shells from a new Explorer window"
        }
    } else {
        "PATH-OK $target was already on the user PATH"
    }
} finally {
    $key.Dispose()
}
$hash = (Get-FileHash (Join-Path $target 'repose.exe')).Hash
"INSTALLED repose.exe sha256 $hash"
